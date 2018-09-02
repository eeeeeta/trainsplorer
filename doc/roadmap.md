# Roadmap

*aka How To Turn This Crazy 'Research Project' Into Something Useful*

- Fix remaining backend problems:
  - Autofix schedule download fails, so we don't have to manually do recovery
  - Maybe make it splurge all the logs to IRC? That might be handy...
- Fix osms-web to not suck:
  - don't choke on multiple train mvts for one schedule mvt
  - handle errors *sanely*
  - support darwin schedules and display 'em nicely
  - support displaying stations
  - support displaying geo paths
  - support displaying problem stations?
  - redesign schedule mvt display
  - allow specifying services that go through two places
- Start actually linking up geodata to schedules in earnest:
  - add API to get mvts passing through a point
  - add nice slippy map of current train schedules?
- Maybe we should/could make something in osms-db for doing common schedule ops?
  - i.e. "given these ScheduleMvts on this date, do all the fancy dedup for me"
  - `handle_movement_list` in `osms-web/src/movements.rs` does this
    - ...but it doesn't handle the new Darwin fanciness yet
  - Crucially this needs to be adapted to work with schedules from two ends obviously,
    so we can do the midpoint calcs
