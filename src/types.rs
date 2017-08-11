use chrono::NaiveTime;
use serde::*;
fn parse_servloc<'de, D>(d: D) -> Result<Vec<ServiceLocation>, D::Error> where D: Deserializer<'de> {
    Deserialize::deserialize(d)
        .map(|x: Option<_>| {
            x.unwrap_or(Vec::new())
        })
}
fn parse_cpr<'de, D>(d: D) -> Result<Vec<AnnoyingCallingPointWrapper>, D::Error> where D: Deserializer<'de> {
    Deserialize::deserialize(d)
        .map(|x: Option<_>| {
            x.unwrap_or(Vec::new())
        })
}

#[derive(Deserialize, Debug)]
pub struct BadRequestReply {
    pub message: String
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StationBoardRaw {
    pub location_name: String,
    pub crs: String,
    pub train_services: Vec<ServiceItemRaw>
}
#[derive(Debug)]
pub struct StationBoard {
    pub location_name: String,
    pub crs: String,
    pub train_services: Vec<ServiceItem>
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServiceItemRaw {
    #[serde(default, deserialize_with = "parse_servloc")]
    pub origin: Vec<ServiceLocation>,
    #[serde(default, deserialize_with = "parse_servloc")]
    pub destination: Vec<ServiceLocation>,
    #[serde(default, deserialize_with = "parse_servloc")]
    pub current_origins: Vec<ServiceLocation>,
    #[serde(default, deserialize_with = "parse_servloc")]
    pub current_destinations: Vec<ServiceLocation>,
    pub sta: Option<String>,
    pub eta: Option<String>,
    pub std: Option<String>,
    pub etd: Option<String>,
    pub rsid: Option<String>,
    #[serde(default, deserialize_with = "parse_cpr")]
    pub previous_calling_points: Vec<AnnoyingCallingPointWrapper>,
    #[serde(default, deserialize_with = "parse_cpr")]
    pub subsequent_calling_points: Vec<AnnoyingCallingPointWrapper>,
    #[serde(rename = "serviceID")]
    pub id: Option<String>
}
#[derive(Debug)]
pub enum TimingInformation {
    OnTime {
        time: NaiveTime,
        actual: bool
    },
    Delayed {
        scheduled: NaiveTime,
        updated: NaiveTime,
        actual: bool
    },
    Scheduled(NaiveTime),
    Cancelled,
    Unknown,
}
impl TimingInformation {
    pub fn scheduled_time(&self) -> Option<&NaiveTime> {
        use self::TimingInformation::*;
        match *self {
            OnTime { ref time, .. } => Some(time),
            Delayed { ref scheduled, .. } => Some(scheduled),
            Scheduled(ref t) => Some(t),
            _ => None,
        }
    }
    pub fn updated_time(&self) -> Option<&NaiveTime> {
        use self::TimingInformation::*;
        match *self {
            Delayed { ref updated, .. } => Some(updated),
            _ => None,
        }
    }
    pub fn best_time(&self) -> Option<&NaiveTime> {
        if let Some(ut) = self.updated_time() {
            Some(ut)
        }
        else {
            self.scheduled_time()
        }
    }
}
#[derive(Debug)]
pub enum OriginOrDestination {
    Original(Vec<ServiceLocation>),
    Current(Vec<ServiceLocation>)
}
impl Into<Vec<ServiceLocation>> for OriginOrDestination {
    fn into(self) -> Vec<ServiceLocation> {
        use self::OriginOrDestination::*;
        match self {
            Original(x) => x,
            Current(x) => x
        }
    }
}
impl OriginOrDestination {
    pub fn is_original(&self) -> bool {
        use self::OriginOrDestination::*;
        match *self {
            Original(_) => true,
            Current(_) => false
        }
    }
    pub fn is_current(&self) -> bool {
        !self.is_original()
    }
}
#[derive(Debug)]
pub struct ServiceItem {
    pub origin: OriginOrDestination,
    pub destination: OriginOrDestination,
    pub arr: TimingInformation,
    pub dep: TimingInformation,
    pub previous_calling_points: Vec<CallingPoint>,
    pub subsequent_calling_points: Vec<CallingPoint>,
    pub rsid: Option<String>,
    pub id: Option<String>
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServiceLocation {
    pub location_name: String,
    pub crs: String
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AnnoyingCallingPointWrapper {
    pub calling_point: Vec<CallingPointRaw>
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CallingPointRaw {
    pub location_name: String,
    pub crs: String,
    pub st: String,
    pub et: Option<String>,
    pub at: Option<String>
}
#[derive(Debug)]
pub struct CallingPoint {
    pub location_name: String,
    pub crs: String,
    pub time: TimingInformation
}
impl From<StationBoardRaw> for StationBoard {
    fn from(sb: StationBoardRaw) -> StationBoard {
        let StationBoardRaw { location_name, crs, train_services } = sb;
        let train_services = train_services.into_iter().map(|x| x.into()).collect();
        StationBoard { location_name, crs, train_services }
    }
}
impl From<ServiceItemRaw> for ServiceItem {
    fn from(r: ServiceItemRaw) -> ServiceItem {
        let ServiceItemRaw {
            origin,
            destination,
            current_origins,
            current_destinations,
            sta, eta, std, etd,
            previous_calling_points,
            subsequent_calling_points,
            id, rsid
        } = r;
        let origin = if current_origins.len() > 0 {
            OriginOrDestination::Current(current_origins)
        } else {
            OriginOrDestination::Original(origin)
        };
        let destination = if current_destinations.len() > 0 {
            OriginOrDestination::Current(current_destinations)
        } else {
            OriginOrDestination::Original(destination)
        };
        let arr = times_to_info(sta, eta, false);
        let dep = times_to_info(std, etd, false);
        let previous_calling_points = previous_calling_points.into_iter()
            .flat_map(|s| s.calling_point)
            .map(|x| x.into()).collect();
        let subsequent_calling_points = subsequent_calling_points.into_iter()
            .flat_map(|s| s.calling_point)
            .map(|x| x.into()).collect();
        ServiceItem { origin, destination, arr, dep, previous_calling_points, subsequent_calling_points, id, rsid }
    }
}
impl From<CallingPointRaw> for CallingPoint {
    fn from(c: CallingPointRaw) -> CallingPoint {
        let CallingPointRaw { location_name, crs, st, et, at } = c;
        let (est, actual) = if at.is_none() { (et, false) } else { (at, true) };
        let time = times_to_info(Some(st), est, actual);
        CallingPoint { location_name, crs, time }
    }
}

fn times_to_info(s: Option<String>, e: Option<String>, actual: bool) -> TimingInformation {
    use self::TimingInformation::*;
    if let Some(s) = s {
        if s == "Cancelled" {
            return Cancelled;
        }
        let s = s.replace("*", "");
        let nt = match NaiveTime::parse_from_str(&s, "%H:%M") {
            Ok(t) => t,
            Err(_) => return Unknown
        };
        if let Some(e) = e {
            if e == "On time" {
                OnTime { time: nt, actual }
            }
            else if e == "Cancelled" {
                Cancelled
            }
            else {
                let ne = match NaiveTime::parse_from_str(&s, "%H:%M") {
                    Ok(t) => t,
                    Err(_) => return Scheduled(nt)
                };
                Delayed { scheduled: nt, updated: ne, actual }
            }
        }
        else {
            Scheduled(nt)
        }
    }
    else {
        Unknown
    }
}
