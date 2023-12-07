# Red Siren


## Development



### Shared

```
cargo build --package shared  
```

```
cargo build --package shared_types
```

### Web (leptos)

```
cd web-leptos
cargo leptos watch
```

### Web (audio worklet)

```
cd worklet
pnpm run dev
```

### iOS

Open `iOS/RedSiren.xcworkspace` with Xcode

Using [cocoapods](https://cocoapods.org/). Run `pod update` in `iOS` directory


### Android

Open `Android/` with Android studio