# Mock fixture

Windows PE ที่ `game.exe` — บน Windows CI `mock-build` จะ build จาก `maxion-protector/examples/hello-world` อัตโนมัติถ้ายังไม่มี

```powershell
# manual (Windows only)
cd maxion-protector/examples/hello-world
cargo build --release
Copy-Item target/release/hello.exe ../../fixtures/mock-game/game.exe
```
