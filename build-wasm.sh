# This shell script provides convenient way to build
# this crate into WebAssembly.
#
# The APIs relates to WebGPU in web-sys ctate is only
# available if the config "web_sys_unstable_apis" is
# defined at present.

export RUSTFLAGS='--cfg=web_sys_unstable_apis'

wasm-pack build --target web
