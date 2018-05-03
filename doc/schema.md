# osms-nrod database schema

(document still *very much* WIP; in future, this will actually be helpful)

## new schema migration TODO

- remember to link up new ScheduleMvts with new stations somehow
- convert osms-nrod and osms-web to use the new schema
- while we're at it, probably worth fixing stations as well

## `Schedule`

- id
- uid (schedule UID)
- start_date, end_date (self-explanatory)
- days (applicable days)
- stp_indicator (schedule type indicator)
- signalling_id (optional headcode)

## `ScheduleAction`

enum, types:

- Arrive
- Depart
- Pass

## `ScheduleMvt`

- id
- parent_sched (references `Schedule` id)
- tiploc
- parent_station (references `Station` id)
- action (a `ScheduleAction`)
- time

## `Train`

- id
- parent_sched (references `Schedule` id)
- trust_id
- date
- signalling_id
- terminated

## `TrainMvt`

- id
- parent_train (references `Train` id)
- parent_mvt (references `ScheduleRecord` id)
- time
- source (probably just a varchar for now describing source)

