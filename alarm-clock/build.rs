fn main() {
	let target = std::env::var("TARGET").unwrap();
	let mut split = target.split('-');

	let arch = split.next().unwrap();
	let _vendor = split.next().unwrap();
	let os = split.next().unwrap();

	if os != "linux" {
		println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
		println!("cargo:rustc-link-arg-bins=-Tlinkall.x");

		if arch == "xtensa" {
			println!(
				"cargo:rustc-check-cfg=cfg(xtensa, values(\"esp32\", \"esp32s2\", \"esp32s3\"))"
			);
			let xtensa = split.next().unwrap();
			println!("cargo:rustc-cfg=xtensa=\"{}\"", xtensa);
		}
	}
}
