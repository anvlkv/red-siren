# Red Siren


## Development

Shared and AuCore are distinct crux cores communicating via current shell's `play` and `resolve` capabilities.

iOS and Android shells are using cores via `bindgen` package.

Web version uses Shared app_core as is (rust), and AuCore via the `worklet` package.

### Cores, types and bindgen

```
cargo build --package app_core  
```

```
cargo build --package aucore  
```

```
cargo build --package typegen
```

```
cargo build --package bindgen
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
pnpm run build
```

### iOS

Open `iOS/RedSiren.xcworkspace` with Xcode.

Requires [cocoapods](https://cocoapods.org/).

Run `pod update` in `iOS` directory.

Rebuild `bindgen` upon interface changes.

### Android

Open `Android/` with Android studio.

Requires [cargo ndk](https://github.com/bbqsrc/cargo-ndk).

Clean build upon rust code changes.

