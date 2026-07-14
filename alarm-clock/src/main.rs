#![cfg_attr(not(target_os = "linux"), no_std, no_main)]
#![feature(impl_trait_in_assoc_type)]

#[cfg(target_os = "linux")]
mod linux;
#[cfg(all(target_os = "none", target_arch = "xtensa"))]
mod xtensa;

#[cfg(target_os = "none")]
#[cfg_attr(target_arch = "xtensa", esp_rtos::main)]
async fn main(spawner: embassy_executor::Spawner) {
	#[cfg(target_arch = "xtensa")]
	xtensa::run(spawner).await.unwrap();
}

#[cfg(target_os = "linux")]
fn main() {
	#[embassy_executor::task]
	async fn start(spawner: embassy_executor::Spawner) {
		#[cfg(target_os = "linux")]
		linux::run(spawner).await.unwrap();
	}

	let executor = Box::leak(Box::new(embassy_executor::Executor::new()));
	executor.run(|spawner| {
		spawner.spawn(start(spawner).unwrap());
	});

}
