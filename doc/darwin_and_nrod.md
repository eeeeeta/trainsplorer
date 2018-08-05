# darwin and nrod linking ideas

(mostly incoherent)

## the problem

- we have a bunch of the following errors with the current (1d4462f) code:

```
Aug 05 11:33:04 osms-ii osms-nrod[30613]: [osms_nrod::darwin ERROR] Failed to process TS: Multiple errors: [NoMovementsFound(468139, [2], ["THTH"], Some(10:32:09)), NoMovementsFound(468139, [2], ["NORBURY"], Some(10:33:04)), NoMovementsFound(468139, [2], ["POUPRTJ"], Some(10:40:09))]
```

- these signify that Darwin gave us predictions/updates for locations that weren't
  in the ITPS schedule for that train.
- we don't implement the Darwin `<schedule>` element yet. we're assuming that this
  is the problem: that Darwin is notifying us of trains making extra stops, and
  we aren't absorbing this information.
- obviously, fixing this implies having two copies of the schedule: the ITPS one,
  and the Darwin one. this sucks, and we need to find a way to merge the two.

## solution I

- extend the Train object to have a new `parent_darwin_sched` field.
- extend the Schedule object to have a new `darwin_id` (RID) field.
- when we get a new `<schedule>` object:
  - activate/link the RID as usual
  - check whether the new `<schedule>` object is just the ITPS schedule. if so,
    stop.
    - this is because it's probably going to be the same quite a lot of the time,
      and storage is eeeeexpensive
    - (also less processing power later on)
  - make a new Schedule with the data from Darwin. set the `darwin_id` to the
    RID of the schedule.
  - set the Train's `parent_darwin_sched` field to the new Schedule's `id`.
- when we get a Darwin train movement:
  - try to find it in the ITPS (old) schedule. if that works, do what we currently
    do.
  - if we can't find it, try and find it in the new Darwin schedule. if that works,
    do what we currently do (set the parent_mvt to the mvt from the Darwin schedule).
- why don't we just use the Darwin schedule for everything?
  - it's probably going to be identical to the ITPS schedule most of the time
  - both Darwin and TRUST log movements against the ITPS schedule currently
    - → we'd lose out on TRUST data for Darwin trains, which we don't want (Darwin
      goes down a lot, remember? :P)
  - we don't have to deal with the train schedule being different from the planned
    schedule (yet) in a lot of applications
- with this method, we end up with a bunch of TrainMvts for each train that
  potentially reference two schedules. how do we merge the two?
  - well, just do what we do with movements
  - create a big array of all the (tiploc, action, time) tuples we have
  - search both the ITPS and the Darwin schedules, preferring the ITPS ones
  - **NB:** if we can't find a (tiploc, action, time) pair in the Darwin schedule,
    drop it (because it's been cancelled or something)
    - that shouldn't really ever happen, and should probably alert loudly, because
      doesn't Darwin have a special 'cancelled' property that it uses instead of
      just nuking locations?
  - the resultant movements become the definitive schedule for that train
- this solution seems pretty okay, to be honest
  - this probably means I'm not rigorous enough
