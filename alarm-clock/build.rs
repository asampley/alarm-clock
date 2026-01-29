fn main() {
	println!("cargo:rustc-link-arg-bins=-Tlinkall.x");
	println!("cargo:rustc-link-arg-bins=-Tdefmt.x");

	let target = std::env::var("TARGET").unwrap();
	let mut split = target.split('-');
	let arch = split.next().unwrap();
	if arch == "xtensa" {
		println!("cargo:rustc-check-cfg=cfg(xtensa, values(\"esp32\", \"esp32s2\", \"esp32s3\"))");
		let xtensa = split.next().unwrap();
		println!("cargo:rustc-cfg=xtensa=\"{}\"", xtensa);
	}
}
