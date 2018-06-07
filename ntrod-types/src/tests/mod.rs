use serde_json;
use {schedule, movements, vstp};

macro_rules! mktest {
    ($($name:ident, $path:expr, $ty:ty),*) => {
        $(
        #[test]
        fn $name() {
            let data = include_str!($path);
            let _: $ty = serde_json::from_str(&data).unwrap();
        }
        )*
    }
}
mktest! {
    parse_schedule_v1, "schedule_v1.json", schedule::Record,
    parse_schedule_v1_2, "schedule_v1_2.json", schedule::Record,
    parse_schedule_v1_3, "schedule_v1_3.json", schedule::Record,
    parse_schedule_v1_4, "schedule_v1_4.json", schedule::Record,
    parse_schedule_v1_5, "schedule_v1_5.json", schedule::Record,
    parse_schedule_v1_6, "schedule_v1_6.json", schedule::Record,
    parse_association_v1, "association_v1.json", schedule::Record,
    parse_timetable_v1, "timetable_v1.json", schedule::Record,
    parse_tiploc_v1, "tiploc_v1.json", schedule::Record,
    parse_vstp_v1, "vstp_v1.json", vstp::Record,
    parse_movements_various_1, "movements_various_1.json", movements::Records,
    parse_movements_various_2, "movements_various_2.json", movements::Records,
    parse_movements_various_3, "movements_various_3.json", movements::Records,
    parse_movements_various_4, "movements_various_4.json", movements::Records,
    parse_movements_0005, "movements_0005.json", movements::Reinstatement,
    parse_movements_0003, "movements_0003.json", movements::Movement,
    parse_movements_0003_2, "movements_0003_2.json", movements::Movement,
    parse_movements_0003_3, "movements_0003_3.json", movements::Movement,
    parse_movements_0003_4, "movements_0003_4.json", movements::Movement,
    parse_movements_0003_5, "movements_0003_5.json", movements::Movement,
    parse_movements_0002, "movements_0002.json", movements::Cancellation,
    parse_movements_0001, "movements_0001.json", movements::Activation
}
