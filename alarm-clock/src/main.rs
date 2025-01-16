#![no_std]
#![no_main]

use embassy_executor::Spawner;

use alarm_clock::info;
use alarm_clock::startup;

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
	info!("Embassy initialized!");

	startup(spawner).await.unwrap();
}
