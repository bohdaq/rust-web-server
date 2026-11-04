# S5 data: the test app on a kube-dc managed PostgreSQL (TLS verify-full with
# the platform CA, RWS_DB_SSL_*), a kube-dc bucket (Ceph RGW, S3Storage), a
# job queue across 3 replicas, the mailer to mailpit. Sourced by run.sh.

todos_ok() {
  local out id
  out=$(curl -sS --max-time 20 -H 'Content-Type: application/json' -d '{"title":"from stage"}' -w '\n%{http_code}' "$APP_URL/app/todos")
  [ "$(tail -1 <<<"$out")" = 201 ] || { echo "    POST: $out" >&2; return 1; }
  id=$(head -1 <<<"$out" | json_get 'd["id"]')
  [ "$(curl -sS --max-time 20 "$APP_URL/app/todos/$id" | json_get 'd["title"]')" = "from stage" ] || return 1
  [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' -X DELETE "$APP_URL/app/todos/$id")" = 204 ] &&
    [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' "$APP_URL/app/todos/$id")" = 404 ]
}
app_check S5.1 "ORM on managed PostgreSQL over verified TLS: create, read, delete" todos_ok
pg_tls_ok() { # every app pod connected with verify-full (no 'retrying' left in the log)
  local pod
  for pod in $(k get pods -l app=e2e-app -o name); do
    # The app's own lines ([e2e] …): a failed connect, TLS refused, unavailable.
    k logs "$pod" 2>/dev/null | grep -i '^\[e2e\].*database' | grep -qiv 'database ready' &&
      { echo "    $pod: $(k logs "$pod" | grep -i '^\[e2e\].*database' | grep -iv 'database ready' | tail -1)" >&2; return 1; }
  done
  return 0
}
app_check S5.2 "no app replica had a database or certificate error" pg_tls_ok

jobs_ok() { # 15 jobs, enqueued through the gateway (any replica), each run once
  local i key runs pending ok=0 pods
  for i in $(seq 15); do curl -sS --max-time 20 -o /dev/null -X POST "$APP_URL/app/jobs?key=s5-$RUN_ID-$i"; done
  for i in $(seq 15); do
    key="s5-$RUN_ID-$i"
    eventually 60 sh -c "curl -sS --max-time 20 $APP_URL/app/jobs/$key | grep -q '\"pending\":0'" || { echo "    $key still pending" >&2; return 1; }
    runs=$(curl -sS --max-time 20 "$APP_URL/app/jobs/$key" | json_get 'len(d["runs"])')
    [ "$runs" = 1 ] && ok=$((ok + 1)) || echo "    $key ran $runs times" >&2
    pods+=" $(curl -sS --max-time 20 "$APP_URL/app/jobs/$key" | json_get 'd["runs"][0]["pod"]' 2>/dev/null)"
  done
  echo "    15 jobs: $ok ran exactly once, on $(tr ' ' '\n' <<<"$pods" | grep . | sort -u | wc -l) pods" >&2
  [ "$ok" = 15 ]
}
app_check S5.3 "jobs: 15 across 3 replicas, each runs exactly once" jobs_ok
retry_ok() {
  local key="s5-$RUN_ID-retry"
  curl -sS --max-time 20 -o /dev/null -X POST "$APP_URL/app/jobs?key=$key&fail_first=1"
  eventually 60 sh -c "curl -sS --max-time 20 $APP_URL/app/jobs/$key | grep -q '\"pending\":0'" || return 1
  [ "$(curl -sS --max-time 20 "$APP_URL/app/jobs/$key" | json_get '(len(d["runs"]), d["runs"][0]["attempts"])')" = "(1, 2)" ]
}
app_check S5.4 "a job that fails once is retried and runs once (attempts 2)" retry_ok

s3_ok() {
  local name="s5-$RUN_ID.bin" src="$REPORT/S5.5.bin"
  head -c 300000 /dev/urandom >"$src"
  [ "$(curl -sS --max-time 30 -o /dev/null -w '%{http_code}' -X PUT --data-binary "@$src" -H 'Content-Type: application/octet-stream' "$APP_URL/app/files/$name")" = 201 ] || return 1
  curl -sS --max-time 30 "$APP_URL/app/files/$name" | cmp -s - "$src" || { echo "    GET differs" >&2; return 1; }
  S5_FILE=$name
}
app_check S5.5 "S3 on a kube-dc bucket: 300 KB put and read back byte for byte" s3_ok
presign_ok() { # the presigned URL names the in-cluster endpoint: fetched from the runner
  local url; url=$(curl -sS --max-time 20 "$APP_URL/app/files/$S5_FILE/presign")
  [ "$(incluster -o /dev/null -w '%{http_code}' "$url")" = 200 ] &&
    [ "$(incluster -o /dev/null -w '%{http_code}' "${url/X-Amz-Signature=/X-Amz-Signature=0}")" = 403 ]
}
app_check S5.6 "a presigned GET works in the cluster; a tampered one 403" presign_ok
s3_delete_ok() {
  [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' -X DELETE "$APP_URL/app/files/$S5_FILE")" = 204 ] &&
    [ "$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' "$APP_URL/app/files/$S5_FILE")" = 404 ]
}
app_check S5.7 "S3 delete, then 404" s3_delete_ok

mail_ok() {
  local subject="s5-$RUN_ID-mail"
  [ "$(curl -sS --max-time 20 -X POST "$APP_URL/app/mail?to=olga@e2e.test&subject=$subject")" = sent ] || return 1
  eventually 30 sh -c "kubectl -n $NS exec deploy/runner -- curl -sS --max-time 10 'http://mailpit:8025/api/v1/search?query=subject:$subject' | grep -q '\"messages_count\":1'"
}
app_check S5.8 "mailer: one mail reaches the SMTP sink" mail_ok
