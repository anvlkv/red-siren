# Red Siren


## Development



### Shared

```
cargo build --package shared  
```

```
cargo build --package aucore  
```

```
cargo build --package shared_types
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


### Android

Open `Android/` with Android studio.

#### aucore (oboe bridge)

Requires [cargo ndk](https://github.com/bbqsrc/cargo-ndk).


