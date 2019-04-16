use std::fmt::Display;

#[derive(Serialize, Deserialize, Copy, Clone, Debug, is_enum_variant, Display)]
#[cfg_attr(feature = "postgres-traits", derive(FromSql, ToSql))]
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
    StpBus,
    #[serde(rename = " ")]
    Empty,
    #[serde(rename = "")]
    None,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Debug, is_enum_variant, Display)]
#[cfg_attr(feature = "postgres-traits", derive(FromSql, ToSql))]
pub enum StpIndicator {
    #[serde(rename = "")]
    None,
    #[serde(rename = "P")]
    Permanent,
    #[serde(rename = "O")]
    Overlay,
    #[serde(rename = "N")]
    NewSchedule,
    #[serde(rename = "C")]
    Cancellation
}
impl StpIndicator {
    pub fn as_char(self) -> char {
        use self::StpIndicator::*;

        match self {
            None => ' ',
            Permanent => 'P',
            Overlay => 'O',
            NewSchedule => 'N',
            Cancellation => 'C'
        }
    }
    pub fn create_type() -> &'static str {
        r#"
DO $$
BEGIN
IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'StpIndicator') THEN
CREATE TYPE "StpIndicator" AS ENUM (
'Cancellation',
'NewSchedule',
'Overlay',
'Permanent',
'None'
);
END IF;
END$$;"#
    }
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug, is_enum_variant, Display)]
#[cfg_attr(feature = "postgres-traits", derive(FromSql, ToSql))]
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
    HighSpeedTrain,
    #[serde(rename = "")]
    None
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug, is_enum_variant, Display)]
#[cfg_attr(feature = "postgres-traits", derive(FromSql, ToSql))]
pub enum TrainCategory {
    #[serde(rename="OL")]
    LondonUnderground,
    #[serde(rename="OU")]
    UnadvertisedOrdinaryPassenger,
    #[serde(rename="OO")]
    OrdinaryPassenger,
    #[serde(rename="OS")]
    StaffTrain,
    #[serde(rename="OW")]
    Mixed,
    #[serde(rename="XC")]
    ChannelTunnel,
    #[serde(rename="XD")]
    SleeperEuropeNightServices,
    #[serde(rename="XI")]
    International,
    #[serde(rename="XR")]
    Motorail,
    #[serde(rename="XU")]
    UnadvertisedExpress,
    #[serde(rename="XX")]
    ExpressPassenger,
    #[serde(rename="XZ")]
    SleeperDomestic,
    #[serde(rename="BR")]
    BusReplacement,
    #[serde(rename="BS")]
    Bus,
    #[serde(rename="SS")]
    Ship,
    #[serde(rename="EE")]
    EmptyCoachingStock,
    #[serde(rename="EL")]
    EcsLondonUnderground,
    #[serde(rename="ES")]
    EcsAndStaff,
    #[serde(rename="JJ")]
    Postal,
    #[serde(rename="PM")]
    PostOfficeControlledParcels,
    #[serde(rename="PP")]
    Parcels,
    #[serde(rename="PV")]
    EmptyNpccs,
    #[serde(rename="DD")]
    Departmental,
    #[serde(rename="DH")]
    CivilEngineer,
    #[serde(rename="DI")]
    MechanicalAndElectricalEngineer,
    #[serde(rename="DQ")]
    Stores,
    #[serde(rename="DT")]
    Test,
    #[serde(rename="DY")]
    SignalAndTelecommunicationsEngineer,
    #[serde(rename="ZB")]
    LocomotiveAndBrakeVan,
    #[serde(rename="ZZ")]
    LightLocomotive,
    #[serde(rename="J2")]
    RfDAutomotiveComponents,
    #[serde(rename="H2")]
    RfDAutomotiveVehicles,
    #[serde(rename="J3")]
    RfDEdibleProducts,
    #[serde(rename="J4")]
    RfDIndustrialMinerals,
    #[serde(rename="J5")]
    RfDChemicals,
    #[serde(rename="J6")]
    RfDBuildingMaterials,
    #[serde(rename="J8")]
    RfDGeneralMerchandise,
    #[serde(rename="H8")]
    RfDEuropean,
    #[serde(rename="J9")]
    RfDFreightlinerContracts,
    #[serde(rename="H9")]
    RfDFreightlinerOther,
    #[serde(rename="A0")]
    Coal,
    #[serde(rename="E0")]
    CoalMGR,
    #[serde(rename="B0")]
    CoalAndNuclear,
    #[serde(rename="B1")]
    Metals,
    #[serde(rename="B4")]
    Aggregates,
    #[serde(rename="B5")]
    DomesticandIndustrialWaste,
    #[serde(rename="B6")]
    BuildingMaterials,
    #[serde(rename="B7")]
    PetroleumProducts,
    #[serde(rename="H0")]
    RfDEct,
    #[serde(rename="H1")]
    RfDEctIntermodal,
    #[serde(rename="H3")]
    RfDEctAutomotive,
    #[serde(rename="H4")]
    RfDEctContractServices,
    #[serde(rename="H5")]
    RfDEctHaulmark,
    #[serde(rename="H6")]
    RfDEctJointVenture,
    #[serde(rename="")]
    None
}
