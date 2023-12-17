# Red Siren


## Development

Shared and AuCore are distinct crux cores communicating via current shell's `play` and `resolve` capabilities.

iOS and Android shells are using cores via `uniffi-bindgen` package.

Web version uses Shared core as is (rust), and AuCore via the `worklet` package.

### Cores, types and bindgen

```
cargo build --package shared  
```

```
cargo build --package aucore  
```

```
cargo build --package shared_types
```

```
cargo build --package uniffi-bindgen
```

### Web (leptos)

```
cd web-leptos
cargo leptos watch
```

#### Web (audio worklet)

Requires [pnpm](https://pnpm.io).
Requires [wasm-pack](https://github.com/rustwasm/wasm-pack).

```
cd web-leptos/worklet
pnpm run dev
```

### iOS

Open `iOS/RedSiren.xcworkspace` with Xcode.

Requires [cocoapods](https://cocoapods.org/).

Run `pod update` in `iOS` directory.

Rebuild `uniffi-bindgen` upon interface changes.

### Android

Open `Android/` with Android studio.

Requires [cargo ndk](https://github.com/bbqsrc/cargo-ndk).

Clean build upon rust code changes.

