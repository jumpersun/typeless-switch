// Synthetic sessions only. Node's UTF-8 Buffer conversion is the interoperability reference.
import { createHash, pbkdf2Sync, createCipheriv } from 'node:crypto';
import { writeFileSync, mkdirSync } from 'node:fs';
const dir = new URL('../src-tauri/tests/fixtures/', import.meta.url);
mkdirSync(dir, { recursive: true });
for (const profile of ['darwin-arm64/Typeless', 'darwin-x64/Typeless', 'win32-x64/Typeless.exe']) {
  for (const name of ['alice', 'bob', 'expired']) {
    const [platform, app] = profile.split('/');
    const iv = Buffer.from([0xff, 0xc3, 0x28, 0xed, 0xa0, 0x80, 0, 1, 5, 0xe2, 0x82, 0xac, 2, 3, 4, 6]);
    const appKey = pbkdf2Sync(createHash('sha256').update(platform).digest('hex') + app, 'typeless-user-service', 10000, 32, 'sha256');
    const key = pbkdf2Sync(appKey, iv.toString(), 10000, 32, 'sha512');
    const jwt = 'e30.' + Buffer.from(JSON.stringify({ exp: name === 'expired' ? 1 : 4102444800 })).toString('base64url') + '.synthetic';
    const plain = JSON.stringify({ userData: JSON.stringify({ user_id: name, email: `${name}@example.test`, access_token: 'synthetic-access-only', refresh_token: jwt }) });
    const cipher = createCipheriv('aes-256-cbc', key, iv);
    writeFileSync(new URL(`${platform}-${name}.bin`, dir), Buffer.concat([iv, Buffer.from(':'), cipher.update(plain), cipher.final()]));
  }
}
