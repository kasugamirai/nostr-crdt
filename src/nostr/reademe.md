# run all test
```bash
wasm-pack test --firefox
```

# run single test
```bash 
wasm-pack test  --firefox -- -- test_get_events
```

# run target file tests
```bash
wasm-pack test --firefox -- -- nostr::fetch
```