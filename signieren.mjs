// Signiert holen.py für das Selbst-Update (Ed25519): Installierte Kopien übernehmen nur Fassungen mit gültiger Signatur,
// auch wenn jemand GitHub oder den Server übernimmt. Der private Schlüssel liegt nur auf diesem PC, nie im Repo.
//   node signieren.mjs --neu   Schlüsselpaar anlegen (einmal); den ausgegebenen öffentlichen Schlüssel in holen.py als SIGNATUR eintragen
//   node signieren.mjs         holen.py so signieren, wie sie gerade im Git-Index steht → holen.py.sig (macht .git/hooks/pre-commit)
import { createPrivateKey, createPublicKey, generateKeyPairSync, sign } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';

const SCHLUESSEL = process.env.LH_SIGNATUR || join(homedir(), '.lernhelfer', 'signatur-privat.pem');
const hex = k => Buffer.from(createPublicKey(k).export({ format: 'jwk' }).x, 'base64url').toString('hex');

if (process.argv[2] === '--neu') {
  if (existsSync(SCHLUESSEL)) throw new Error(`Gibt es schon, nicht überschreiben: ${SCHLUESSEL}`);
  const { privateKey } = generateKeyPairSync('ed25519');
  mkdirSync(dirname(SCHLUESSEL), { recursive: true });
  writeFileSync(SCHLUESSEL, privateKey.export({ type: 'pkcs8', format: 'pem' }), { mode: 0o600, flag: 'wx' });
  console.log(`Privater Schlüssel: ${SCHLUESSEL} (sichern: USB-Stick oder Passwort-Manager)\nÖffentlich: ${hex(privateKey)}`);
} else {
  const ordner = dirname(new URL(import.meta.url).pathname.replace(/^\/(\w:)/, '$1'));
  const schluessel = createPrivateKey(readFileSync(SCHLUESSEL));
  const holen = readFileSync(join(ordner, 'holen.py'), 'utf8');
  const eingetragen = holen.match(/^SIGNATUR = "([0-9a-f]{64})"/m)?.[1];
  if (eingetragen !== hex(schluessel)) throw new Error('SIGNATUR in holen.py passt nicht zum privaten Schlüssel');
  const daten = execFileSync('git', ['show', ':holen.py'], { cwd: ordner, maxBuffer: 1 << 26 });  // genau die Bytes, die GitHub und der Server ausliefern
  writeFileSync(join(ordner, 'holen.py.sig'), sign(null, daten, schluessel).toString('base64') + '\n');
}
