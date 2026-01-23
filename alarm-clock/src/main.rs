#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

use embassy_executor::Spawner;

use alarm_clock::info;
use alarm_clock::startup;

#[cfg(target_arch = "xtensa")]
esp_bootloader_esp_idf::esp_app_desc!();

#[cfg_attr(target_arch = "xtensa", esp_rtos::main)]
async fn main(spawner: Spawner) {
	info!("Embassy initialized!");

	startup(spawner).await.unwrap();
}
