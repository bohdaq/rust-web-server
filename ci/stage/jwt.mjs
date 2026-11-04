// RS256 / ES256 JWTs for S3 (Node 24, no packages):
//   node jwt.mjs <RS256|ES256> <private key PEM file> <sub> <exp unix time>
import { readFileSync } from "node:fs";
import { createSign } from "node:crypto";
const [alg, keyFile, sub, exp] = process.argv.slice(2);
const b64 = (v) => Buffer.from(typeof v === "string" ? v : JSON.stringify(v)).toString("base64url");
const input = `${b64({ alg, typ: "JWT" })}.${b64({ sub, exp: Number(exp) })}`;
const signer = createSign("SHA256");
signer.update(input);
// JWS ES256 wants r||s (IEEE P1363), not DER.
const sig = signer.sign({ key: readFileSync(keyFile), dsaEncoding: "ieee-p1363" }).toString("base64url");
console.log(`${input}.${sig}`);
