import { LitElement, css, html } from "lit";

/** The notes page: lists, adds and deletes notes through /api/notes. */
export class NotesApp extends LitElement {
  static properties = {
    notes: { state: true },
    error: { state: true },
  };

  static styles = css`
    :host {
      display: block;
      max-width: 40rem;
      margin: 2rem auto;
      padding: 0 1rem;
      font-family: system-ui, sans-serif;
      color: var(--md-sys-color-on-surface, #1d1b20);
    }
    form {
      display: flex;
      gap: 0.5rem;
      margin-bottom: 1rem;
    }
    input {
      flex: 1;
      font: inherit;
      padding: 0.5rem 0.75rem;
      border: 1px solid var(--md-sys-color-outline, #79747e);
      border-radius: 0.5rem;
      background: transparent;
      color: inherit;
    }
    ul {
      list-style: none;
      padding: 0;
    }
    li {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 0.25rem 0;
      border-bottom: 1px solid var(--md-sys-color-outline-variant, #cac4d0);
    }
    .error {
      color: var(--md-sys-color-error, #b3261e);
    }
  `;

  constructor() {
    super();
    this.notes = [];
    this.error = "";
  }

  connectedCallback() {
    super.connectedCallback();
    this.load();
  }

  async request(method, path, body) {
    const response = await fetch(path, {
      method,
      headers: body ? { "content-type": "application/json" } : {},
      body: body ? JSON.stringify(body) : undefined,
    });
    const data = await response.json();
    if (!response.ok) throw new Error(data.error || response.statusText);
    return data;
  }

  async load() {
    try {
      this.notes = await this.request("GET", "/api/notes");
      this.error = "";
    } catch (e) {
      this.error = e.message;
    }
  }

  async add(event) {
    event.preventDefault();
    const input = this.renderRoot.querySelector("input");
    const text = input.value.trim();
    if (!text) return;
    try {
      await this.request("POST", "/api/notes", { text });
      input.value = "";
      await this.load();
    } catch (e) {
      this.error = e.message;
    }
  }

  async remove(id) {
    try {
      await this.request("DELETE", `/api/notes/${id}`);
      await this.load();
    } catch (e) {
      this.error = e.message;
    }
  }

  render() {
    return html`
      <h1>Notes</h1>
      <form @submit=${this.add}>
        <input aria-label="New note" placeholder="Write a note" />
        <lit-material-button type="submit">Add</lit-material-button>
      </form>
      ${this.error ? html`<p class="error">${this.error}</p>` : ""}
      <ul>
        ${this.notes.map(
          (note) => html`<li>
            <span>${note.text}</span>
            <lit-material-button variant="text" @click=${() => this.remove(note.id)}>Delete</lit-material-button>
          </li>`,
        )}
      </ul>
    `;
  }
}

customElements.define("notes-app", NotesApp);
