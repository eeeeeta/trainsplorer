# osms-nrod database schema

(document still *very much* WIP; in future, this will actually be helpful)

## todo (perf)

- we amass a whole crapton (like, 19 million) train and schedule mvts over time.
  this is less than great for perf.
  - partial indexing? partitioning? denormalisation?

## todo (osms-web)

- make the map display station paths as well / toggle different layers being displayed
- have a 'problems' bit displaying dodgy stuff:
  - station problems
  - station nav problems
  - not-fully-linked schedules
  - etc

## linking NROD stuff to geo stuff

- grab railtrack data from OSM
  - literally just import of nodes/ways
  - oh yeah, also crossings would be helpful
- grab tiploc -> lat,lon mappings from naptan data
- make naptan tiplocs into stations:
  - just look for nodes in the nearest ~50-100m?
  - deduplicate nodes that are connected to one another, so we only join onto tracks once
  - if we didn't find any tracks, yell loudly (i.e. plop something in some reporting table)
  - go and change all the tiplocs of the ScheduleMvts to refer to the station we just created
  - override / redo options:
    - allow redoing track search with greater radius
    - allow specifying specific geo areas for given stations, so we can override this crappy process if needed
  - we need to QC this process somehow; probably in the next step
    - as a quick QC step here, we should yell loudly if we didn't connect ourselves to any useful graphpart - ie
      we should probably make the graphpart finder thing check for stations as well (which'll make it SLOOWW but meh)
      and yell if you have a graphpart with fewer than 2 stations, as that's clearly not going to work very well#
    - actually, wait, do graphpart analysis /before/ making stations. then we can very quickly tell which stations are screwed
      as we're adding them, and yell loudly if the tracks we're connecting to seem completely isolated.
- route between stations in schedules
  - iterate through ScheduleMvts of every schedule
  - find pairs of ScheduleMvts that both have a parent_station
  - route between them using navigation API, which helpfully makes stationpaths
  - chuck the resultant stationpath into the starts- and ends_path fields.
  - if we fail to navigate, yell loudly (i.e. plop something in some reporting table). that should definitely not happen.
    - we need some way to ignore troublesome stations - in fact, if we fail to navigate between a certain pair, we should
      definitely store that as a minimum, to avoid redoing a bunch of work and for reporting reasons
    - in fact, look at the station nav table below
    - also probably a good idea to check whether we pass any extra stations, and yell loudly if that's the case
  - this needs to be linked to the whole pair-finding bollocks; you know what I mean
  - this process needs to be redoable, in the event of navigation failures or new tiplocs
    - for a first cut, we can just add a `generation` field to every schedule, and use that for redos:
      they start with generation 0, and you increment the generation every time you change the geodata, then reprocess all
      the things with a generation less than current
    - also we can get a `last_successful_generation` field perhaps, which we only increment when all of the pathfinding
      goes A-OK
- then, to find trains passing thru a given point, we need to do some horrifyingly large JOIN query
  - find the stationpaths passing thru a given point
  - then find the ScheduleMvts with that stationpath as starts- and ends_path
  - ...whose schedules are actually relevant
  - then find the TrainMvts inheriting from those ScheduleMvts

### idea for `StationProblem`

- id
- generation
- parent_station (unique)
- problem type (int)

enum values:

- 0: not connected to any tracks
- 1: isolated graphpart

### idea for `StationNavigationProblem`

fields:

- id
- generation
- origin_station
- destination_station
- problem type (int)
- problem description (varchar)

enum values:

- 0: different graph parts
- 1: passed an additional station
- 2: other failure

## better NROD data TODO

- interpret the TD feed's stuff and the SMART data, and use it to build a better TRUST
- actually do predictions :P
  - add time_delta to trains, update it when we get data in
  - use the time_delta when we don't have a TrainMvt for a given ScheduleMvt, i.e. build 'shadow TrainMvts'
    with that time_delta information
- maybe look at darwin pushport at some point, if we want to wade through a crapton of XML

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

### new fields we gotta add

- generation (how up-to-date this thing's geo data is)

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

### new fields we gotta add

- starts_path (references a `StationPath` id)
- ends_path (references a `StationPath` id)

(also ensure parent_station is RESTRICT)

## `Train`

- id
- parent_sched (references `Schedule` id)
- trust_id
- date
- signalling_id
- terminated

### new fields we gotta add

- time_delta (i.e. how far behind/ahead schedule this train is running; for prediction purposes)

## `TrainMvt`

- id
- parent_train (references `Train` id)
- parent_mvt (references `ScheduleRecord` id)
- time
- source (probably just a varchar for now describing source)

