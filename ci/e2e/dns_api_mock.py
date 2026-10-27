"""The parts of the Cloudflare and Route 53 APIs rws uses, writing the records
into a real DNS server with nsupdate, so a CA (Pebble) can check them.

  python3 -I dns_api_mock.py <nsupdate> <dns server port> <tsig key name> <tsig secret>
                             <cloudflare port> <cloudflare token>
                             <route53 port> <aws access key id> <aws secret access key>

Zone: example.test. Cloudflare requests need `Authorization: Bearer <token>`;
Route 53 requests a valid AWS Signature Version 4 (checked here with an
implementation of its own). Every request is printed, one line each.
"""
import hashlib
import hmac
import http.server
import itertools
import json
import subprocess
import sys
import threading
import urllib.parse

(NSUPDATE, DNS_PORT, KEY_NAME, KEY_SECRET, CF_PORT, CF_TOKEN, R53_PORT, AWS_KEY, AWS_SECRET) = sys.argv[1:10]
ZONE = "example.test"
ids = itertools.count(1)
records = {}  # Cloudflare record id -> (name, content)
lock = threading.Lock()


def nsupdate(*lines):
    script = f"server 127.0.0.1 {DNS_PORT}\nzone {ZONE}\n" + "".join(f"{line}\n" for line in lines) + "send\n"
    subprocess.run([NSUPDATE, "-y", f"hmac-sha256:{KEY_NAME}:{KEY_SECRET}"], input=script.encode(), check=True)


def quoted(value):
    return '"' + value.strip('"') + '"'


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def body(self):
        length = int(self.headers.get("Content-Length") or 0)
        return self.rfile.read(length) if length else b""

    def reply(self, status, body, content_type):
        data = body.encode()
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        self.handle_request("GET")

    def do_POST(self):
        self.handle_request("POST")

    def do_DELETE(self):
        self.handle_request("DELETE")


class Cloudflare(Handler):
    def handle_request(self, method):
        body = self.body()
        url = urllib.parse.urlparse(self.path)
        query = dict(urllib.parse.parse_qsl(url.query))
        print(f"cloudflare {method} {self.path}", flush=True)
        if self.headers.get("Authorization") != f"Bearer {CF_TOKEN}":
            return self.json(403, {"success": False, "errors": [{"code": 10000, "message": "Authentication error"}], "result": None})
        parts = url.path.removeprefix("/client/v4/").strip("/").split("/")
        if parts == ["zones"] and method == "GET":
            found = [{"id": "zone-example-test", "name": ZONE}] if query.get("name") == ZONE else []
            return self.json(200, {"success": True, "result": found})
        if parts[:3] == ["zones", "zone-example-test", "dns_records"]:
            with lock:
                if method == "GET":
                    found = [{"id": i, "type": "TXT", "name": n, "content": c} for i, (n, c) in records.items() if n == query.get("name")]
                    return self.json(200, {"success": True, "result": found})
                if method == "POST":
                    record = json.loads(body)
                    record_id = f"rec{next(ids)}"
                    records[record_id] = (record["name"], record["content"])
                    nsupdate(f"update add {record['name']} {record['ttl']} TXT {quoted(record['content'])}")
                    return self.json(200, {"success": True, "result": {"id": record_id}})
                if method == "DELETE" and len(parts) == 4 and parts[3] in records:
                    name, content = records.pop(parts[3])
                    nsupdate(f"update delete {name} TXT {quoted(content)}")
                    return self.json(200, {"success": True, "result": {"id": parts[3]}})
        self.json(404, {"success": False, "errors": [{"code": 7003, "message": "No route for that URI"}], "result": None})

    def json(self, status, value):
        self.reply(status, json.dumps(value), "application/json")


def signature_valid(handler, method, payload):
    """AWS Signature Version 4, checked from the request as received."""
    auth = handler.headers.get("Authorization", "")
    if not auth.startswith("AWS4-HMAC-SHA256 "):
        return False
    fields = dict(part.strip().split("=", 1) for part in auth[len("AWS4-HMAC-SHA256 "):].split(","))
    access_key, date, region, service, terminator = fields["Credential"].split("/")
    if access_key != AWS_KEY or terminator != "aws4_request":
        return False
    url = urllib.parse.urlparse(handler.path)
    query = "&".join(
        f"{urllib.parse.quote(k, safe='-_.~')}={urllib.parse.quote(v, safe='-_.~')}"
        for k, v in sorted(urllib.parse.parse_qsl(url.query, keep_blank_values=True))
    )
    signed = fields["SignedHeaders"].split(";")
    canonical_headers = "".join(f"{name}:{handler.headers.get(name, '').strip()}\n" for name in signed)
    canonical = "\n".join([method, url.path, query, canonical_headers, ";".join(signed), hashlib.sha256(payload).hexdigest()])
    amz_date = handler.headers.get("X-Amz-Date", "")
    scope = f"{date}/{region}/{service}/aws4_request"
    to_sign = "\n".join(["AWS4-HMAC-SHA256", amz_date, scope, hashlib.sha256(canonical.encode()).hexdigest()])
    key = ("AWS4" + AWS_SECRET).encode()
    for part in (date, region, service, "aws4_request"):
        key = hmac.new(key, part.encode(), hashlib.sha256).digest()
    expected = hmac.new(key, to_sign.encode(), hashlib.sha256).hexdigest()
    return "host" in signed and "x-amz-date" in signed and hmac.compare_digest(expected, fields["Signature"])


def xml_text(xml, tag):
    start = xml.find(f"<{tag}>")
    return xml[start + len(tag) + 2 : xml.find(f"</{tag}>", start)] if start >= 0 else ""


class Route53(Handler):
    def handle_request(self, method):
        payload = self.body()
        url = urllib.parse.urlparse(self.path)
        query = dict(urllib.parse.parse_qsl(url.query))
        print(f"route53 {method} {self.path}", flush=True)
        if not signature_valid(self, method, payload):
            print("route53 SignatureDoesNotMatch", flush=True)
            return self.xml(403, "<ErrorResponse><Error><Type>Sender</Type><Code>SignatureDoesNotMatch</Code>"
                            "<Message>The request signature we calculated does not match the signature you provided.</Message></Error></ErrorResponse>")
        if url.path == "/2013-04-01/hostedzonesbyname" and method == "GET":
            # Like Route 53: zones in order from dnsname — example.test even when asked for another name.
            return self.xml(200, f"<ListHostedZonesByNameResponse><HostedZones><HostedZone><Id>/hostedzone/ZEXAMPLE</Id>"
                            f"<Name>{ZONE}.</Name></HostedZone></HostedZones><DNSName>{query.get('dnsname', '')}</DNSName>"
                            "<MaxItems>1</MaxItems><IsTruncated>false</IsTruncated></ListHostedZonesByNameResponse>")
        if url.path == "/2013-04-01/hostedzone/ZEXAMPLE/rrset" and method == "POST":
            body = payload.decode()
            action, name, value = xml_text(body, "Action"), xml_text(body, "Name"), xml_text(body, "Value")
            if action == "UPSERT":
                nsupdate(f"update delete {name} TXT", f"update add {name} {xml_text(body, 'TTL')} TXT {value}")
            elif action == "DELETE":
                nsupdate(f"update delete {name} TXT {value}")
            return self.xml(200, "<ChangeResourceRecordSetsResponse><ChangeInfo><Id>/change/C1</Id><Status>PENDING</Status>"
                            "</ChangeInfo></ChangeResourceRecordSetsResponse>")
        if url.path == "/2013-04-01/change/C1" and method == "GET":
            return self.xml(200, "<GetChangeResponse><ChangeInfo><Id>/change/C1</Id><Status>INSYNC</Status></ChangeInfo></GetChangeResponse>")
        self.xml(404, "<ErrorResponse><Error><Code>NoSuchHostedZone</Code><Message>not found</Message></Error></ErrorResponse>")

    def xml(self, status, body):
        self.reply(status, body, "text/xml")


servers = [
    http.server.ThreadingHTTPServer(("127.0.0.1", int(CF_PORT)), Cloudflare),
    http.server.ThreadingHTTPServer(("127.0.0.1", int(R53_PORT)), Route53),
]
for server in servers[1:]:
    threading.Thread(target=server.serve_forever, daemon=True).start()
print("mock APIs ready", flush=True)
servers[0].serve_forever()
