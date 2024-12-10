# anthic-service


## Docker build

```
docker build -t antic-service .
```

## Docker run 

```
docker run -p 8080:8080 -e ANTHIC_API_KEY=<VALUE> -e PRIVATE_KEY=<VALUE> antic-service
```

## Direct build
```
cargo build --release
```

## Direct run
```
./target/release/anthic-service
```

