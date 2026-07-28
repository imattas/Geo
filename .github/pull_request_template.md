## Summary

- 

## Verification

- [ ] `cargo fmt --check`
- [ ] `cargo test --workspace --locked`
- [ ] `cargo run --quiet -- check examples/return_42.geo --target x86_64-linux`
- [ ] `cargo run --quiet -- emit-obj examples/object_backend.geo --target x86_64-linux -o target/ci-object-backend-linux.o`
- [ ] `cargo run --quiet -- emit-asm examples/return_42.geo --target x86_64-windows -o target/ci-return-42-windows.asm`

## Notes

- 
