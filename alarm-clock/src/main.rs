#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

mod xtensa;

#[cfg_attr(target_arch = "xtensa", esp_rtos::main)]
async fn main(spawner: embassy_executor::Spawner) {
	#[cfg(target_arch = "xtensa")]
	xtensa::run(spawner).await.unwrap();
}
