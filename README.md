# Red Siren


## Development



### Shared

```
cargo build --package shared  
    Finished dev [unoptimized + debuginfo] target(s) in 10.58s
```

```
cargo build --package shared_types
    Finished dev [unoptimized + debuginfo] target(s) in 10.58s
```

### Web (leptos)

```
cd web-leptos
cargo leptos watch
```

### iOS

Open `iOS/RedSiren.xcodeproj` with Xcode. Adjust the [swift tools version if necessary](https://github.com/redbadger/crux/issues/152) 

### Android

Open `Android/` with Android studio