//! Stored tasks, their stream subscribers, and [`TaskHandle`], which a
//! handler keeps to update a task from another thread.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use super::model::{self, Obj};
use super::{Artifact, Message, Task, TaskState, TaskStatus, MAX_TASKS};
use super::push::{self, PushConfig};
use crate::sse::{SseEvent, SseSender};
use std::sync::mpsc::SyncSender;
use std::sync::OnceLock;

/// Streams per task; more subscribers are refused.
#[doc(hidden)]
pub const MAX_SUBSCRIBERS: usize = 16;

#[doc(hidden)]
pub struct Subscriber {
    #[doc(hidden)]
    pub sender: SseSender,
    /// The JSON-RPC id the stream's events answer.
    #[doc(hidden)]
    pub rpc_id: String,
}

#[doc(hidden)]
pub struct Stored {
    #[doc(hidden)]
    pub owner: String,
    #[doc(hidden)]
    pub task: Task,
    #[doc(hidden)]
    pub subscribers: Vec<Subscriber>,
    #[doc(hidden)]
    pub canceled: Arc<AtomicBool>,
    #[doc(hidden)]
    pub push: Vec<PushConfig>,
}

#[derive(Default)]
#[doc(hidden)]
pub struct TaskStore {
    #[doc(hidden)]
    pub tasks: HashMap<String, Stored>,
    #[doc(hidden)]
    pub order: VecDeque<String>,
}

impl TaskStore {
    /// Store `task` (replacing its earlier state, keeping subscribers), and
    /// drop the oldest finished tasks past [`MAX_TASKS`].
    #[doc(hidden)]
    pub fn put(&mut self, owner: &str, task: Task) {
        match self.tasks.get_mut(&task.id) {
            Some(stored) => stored.task = task,
            None => {
                self.order.push_back(task.id.clone());
                self.tasks.insert(task.id.clone(), Stored { owner: owner.to_string(), task, subscribers: Vec::new(), canceled: Arc::new(AtomicBool::new(false)), push: Vec::new() });
            }
        }
        while self.tasks.len() > MAX_TASKS {
            let finished = self.order.iter().position(|id| self.tasks.get(id).is_none_or(|s| s.task.status.state.is_terminal())).unwrap_or(0);
            match self.order.remove(finished) {
                Some(id) => {
                    self.tasks.remove(&id);
                }
                None => break,
            }
        }
    }

    #[doc(hidden)]
    pub fn get(&self, owner: &str, id: &str) -> Option<&Task> {
        self.tasks.get(id).filter(|s| s.owner == owner).map(|s| &s.task)
    }

    #[doc(hidden)]
    pub fn get_mut(&mut self, owner: &str, id: &str) -> Option<&mut Stored> {
        self.tasks.get_mut(id).filter(|s| s.owner == owner)
    }
}

/// The store and the signal for whoever waits on a task.
#[derive(Default)]
#[doc(hidden)]
pub struct Shared {
    #[doc(hidden)]
    pub store: Mutex<TaskStore>,
    #[doc(hidden)]
    pub changed: Condvar,
    /// The push delivery queue, once push notifications are on.
    #[doc(hidden)]
    pub push: OnceLock<SyncSender<push::Delivery>>,
}

impl Shared {
    #[doc(hidden)]
    pub fn lock(&self) -> MutexGuard<'_, TaskStore> {
        self.store.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// One stream event: a JSON-RPC response whose result is a
/// `StreamResponse` (`task`, `message`, `statusUpdate` or `artifactUpdate`).
#[doc(hidden)]
pub fn event(rpc_id: &str, kind: &str, json: &str) -> SseEvent {
    SseEvent::data(format!(r#"{{"jsonrpc":"2.0","id":{rpc_id},"result":{{"{kind}":{json}}}}}"#))
}

/// Send to every subscriber (one whose buffer is full or who left is
/// dropped: it can subscribe again and gets the whole task first) and queue
/// the task's webhooks.
#[doc(hidden)]
pub fn publish(stored: &mut Stored, queue: Option<&SyncSender<push::Delivery>>, kind: &str, json: &str) {
    stored.subscribers.retain(|s| matches!(s.sender.try_send(event(&s.rpc_id, kind, json)), Ok(true)));
    if let Some(queue) = queue {
        for config in &stored.push {
            push::enqueue(queue, config, kind, json);
        }
    }
}

/// Update a stored task to `state` (unless it already finished), tell its
/// subscribers, end their streams when the task finishes or waits on the
/// client, and wake whoever waits on it. `false` when the task is gone or
/// already finished.
#[doc(hidden)]
pub fn set_state(shared: &Shared, owner: &str, id: &str, state: TaskState, message: Option<Message>) -> bool {
    let mut store = shared.lock();
    let Some(stored) = store.get_mut(owner, id) else { return false };
    if stored.task.status.state.is_terminal() {
        return false;
    }
    let message = message.map(|mut m| {
        m.task_id = Some(stored.task.id.clone());
        m.context_id = Some(stored.task.context_id.clone());
        m
    });
    if let Some(m) = &message {
        stored.task.history.push(m.clone());
    }
    stored.task.status = TaskStatus { state, message, timestamp: model::timestamp() };
    let update = Obj::new().str("taskId", &stored.task.id).str("contextId", &stored.task.context_id).raw("status", &stored.task.status.to_json()).done();
    publish(stored, shared.push.get(), "statusUpdate", &update);
    if state.is_terminal() || state.is_interrupted() {
        stored.subscribers.clear();
    }
    if state == TaskState::Canceled {
        stored.canceled.store(true, Ordering::Relaxed);
    }
    drop(store);
    shared.changed.notify_all();
    true
}

/// A task being worked on in the background: keep it (it's `Clone` and
/// `Send`), report progress, and finish it. Updates after the task finished
/// (or was canceled) are ignored.
#[derive(Clone)]
pub struct TaskHandle {
    #[doc(hidden)]
    pub shared: Arc<Shared>,
    #[doc(hidden)]
    pub owner: String,
    #[doc(hidden)]
    pub id: String,
    #[doc(hidden)]
    pub canceled: Arc<AtomicBool>,
}

impl TaskHandle {
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Working, with an optional progress note to the client.
    pub fn working(&self, note: Option<&str>) {
        set_state(&self.shared, &self.owner, &self.id, TaskState::Working, note.map(Message::agent_text));
    }

    /// Add an output; subscribers get it at once.
    pub fn artifact(&self, artifact: Artifact) {
        self.artifact_chunk(artifact, false, true);
    }

    /// Stream an output in pieces: `append` adds these parts to the
    /// artifact with the same id (sent earlier), `last_chunk` marks the end.
    pub fn artifact_chunk(&self, artifact: Artifact, append: bool, last_chunk: bool) {
        let mut store = self.shared.lock();
        let Some(stored) = store.get_mut(&self.owner, &self.id) else { return };
        if stored.task.status.state.is_terminal() {
            return;
        }
        let update = Obj::new()
            .str("taskId", &stored.task.id)
            .str("contextId", &stored.task.context_id)
            .raw("artifact", &artifact.to_json())
            .raw("append", if append { "true" } else { "false" })
            .raw("lastChunk", if last_chunk { "true" } else { "false" })
            .done();
        match stored.task.artifacts.iter_mut().find(|a| a.artifact_id == artifact.artifact_id) {
            Some(existing) if append => existing.parts.extend(artifact.parts),
            Some(existing) => *existing = artifact,
            None => stored.task.artifacts.push(artifact),
        }
        publish(stored, self.shared.push.get(), "artifactUpdate", &update);
    }

    /// Done.
    pub fn complete(&self) {
        set_state(&self.shared, &self.owner, &self.id, TaskState::Completed, None);
    }

    /// Done with a closing message.
    pub fn complete_with(&self, message: Message) {
        set_state(&self.shared, &self.owner, &self.id, TaskState::Completed, Some(message));
    }

    /// Failed, saying why.
    pub fn fail(&self, reason: &str) {
        set_state(&self.shared, &self.owner, &self.id, TaskState::Failed, Some(Message::agent_text(reason)));
    }

    /// Waiting for the client to answer `question` (its next message
    /// carries this task's id).
    pub fn input_required(&self, question: &str) {
        set_state(&self.shared, &self.owner, &self.id, TaskState::InputRequired, Some(Message::agent_text(question)));
    }

    /// The client canceled the task: stop working on it.
    pub fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::Relaxed)
    }

    /// The task as it is now.
    pub fn snapshot(&self) -> Option<Task> {
        self.shared.lock().get(&self.owner, &self.id).cloned()
    }
}
