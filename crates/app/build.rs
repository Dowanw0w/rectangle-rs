fn main() {
    // Allows opting into a console subsystem build for debugging:
    //   RUSTFLAGS="--cfg rectangle_rs_console" cargo build --release
    println!("cargo::rustc-check-cfg=cfg(rectangle_rs_console)");
}
