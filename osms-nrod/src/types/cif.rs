#[derive(Serialize, Deserialize, Debug)]
pub enum TrainStatus {
    #[serde(rename = "B")]
    Bus,
    #[serde(rename = "F")]
    Freight,
    #[serde(rename = "P")]
    PassengerAndParcels,
    #[serde(rename = "S")]
    Ship,
    #[serde(rename = "T")]
    Trip,
    #[serde(rename = "1")]
    StpPassengerAndParcels,
    #[serde(rename = "2")]
    StpFreight,
    #[serde(rename = "3")]
    StpTrip,
    #[serde(rename = "4")]
    StpShip,
    #[serde(rename = "5")]
    StpBus
}

#[derive(Serialize, Deserialize, Debug)]
pub enum StpIndicator {
    #[serde(rename = "C")]
    Cancellation,
    #[serde(rename = "N")]
    NewSchedule,
    #[serde(rename = "O")]
    Overlay,
    #[serde(rename = "P")]
    Permanent
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PowerType {
    #[serde(rename = "D")]
    Diesel,
    #[serde(rename = "DEM")]
    DieselElectricMultipleUnit,
    #[serde(rename = "DMU")]
    DieselMechanicalMultipleUnit,
    #[serde(rename = "E")]
    Electric,
    #[serde(rename = "ED")]
    ElectroDiesel,
    #[serde(rename = "EML")]
    EmuPlusLocomotive,
    #[serde(rename = "EMU")]
    ElectricMultipleUnit,
    #[serde(rename = "HST")]
    HighSpeedTrain
}
