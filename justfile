id := "st.lynx.plugins.opendeck-akp153.sdPlugin"

package: build-linux build-mac build-win collect zip

build-linux:
    cargo build --release --target x86_64-unknown-linux-gnu --target-dir target/plugin-linux

build-mac:
    docker run --rm -it -v $(pwd):/io -w /io ghcr.io/rust-cross/cargo-zigbuild:sha-eba2d7e cargo zigbuild --release --target universal2-apple-darwin --target-dir target/plugin-mac

build-win:
    cargo build --release --target x86_64-pc-windows-gnu --target-dir target/plugin-win

clean:
    sudo rm -rf target/

collect:
    rm -rf build
    mkdir -p build/{{id}}
    cp -r assets build/{{id}}
    cp manifest.json build/{{id}}
    cp target/plugin-linux/x86_64-unknown-linux-gnu/release/opendeck-akp153 build/{{id}}/opendeck-akp153-linux
    cp target/plugin-mac/universal2-apple-darwin/release/opendeck-akp153 build/{{id}}/opendeck-akp153-mac
    cp target/plugin-win/x86_64-pc-windows-gnu/release/opendeck-akp153.exe build/{{id}}/opendeck-akp153-win.exe

[working-directory: "build"]
zip:
    zip -r opendeck-akp153.plugin.zip {{id}}/
