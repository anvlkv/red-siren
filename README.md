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

Open `iOS/RedSiren.xcworkspace` with Xcode. Adjust the [swift tools version if necessary](https://github.com/redbadger/crux/issues/152) 

Using [cocoapods](https://cocoapods.org/). Run `pod update` in `iOS` directory



### Android

Open `Android/` with Android studio