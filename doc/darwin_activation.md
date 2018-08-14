# A Guide to Darwin Activation

## Intro

In the good old days, we just used TRUST for everything. Train activations came down, we matched them up to ITPS schedules,
and everything was easy and good. However, at some point we decided that we'd also like the predictions and extra data from Darwin
as well - thus, we needed a way to connect the two data sources. More annoyingly, it was frequently the case that Darwin would
issue its predictions before the TRUST train activation. We therefore needed a way for Darwin to activate trains before TRUST did,
leading to the 'Darwin activation' code.

## More expansion on why this is necessary

- Why don't you just use the Darwin schedule and avoid this whole 'Darwin activation'
  stuff?
  - Because, half the time, the Darwin schedule is the same as the ITPS schedule and
    we'd like to avoid storing the same thing twice (and having to deal with this
    case later on).
  - Because we don't necessarily /have/ all of the Darwin schedules, except the ones
    we get via Push Port. ITPS is our main schedule source.
  - Because, at the time Darwin activation was conceived, we couldn't process Darwin
    schedules yet.

## How a Darwin activation works

A Darwin activation tries to essentially predict what TRUST would have done, given
less data than TRUST has. Darwin gives us the date on which a schedule runs (`ssd`)
and its UID (`uid`), and we have to work it out from there. The algorithm has to
be rather conservative: if it chooses a different schedule than TRUST, we'll
end up with duplicate trains, which sucks a whole bunch.

In order to find a train for a RID, UID, and SSD, we currently (as of `6ffb299`):

- check whether we've already dealt with this RID and linked it before ('prelinked')
  - if so, return the train with the matching RID
- check whether there are any activated trains with a `parent_sched` with the same `uid` and a valid `start_date` running on a date equal to `ssd` - i.e. whether TRUST has beat us to it and activated the train first
  - if so, link the train and return it
- if there are no 'obvious' answers, we then have to find a matching schedule ourselves!
- we search for schedules with a matching `uid` that are active on `ssd`.
  - currently, we have to limit ourselves to TRUST schedules (`source = 0`), because
    we don't yet know whether the schedule is VSTP or not, and we know there won't
    be any conflicting schedules if we limit ourselves to one source.
  - we then find the authoritative schedule from this set, and abort if there isn't
    one or if there are >2 conflicting schedules.
- then, we upsert the train (remember, TRUST may have activated the train in the meantime).
  - upsert works because of the UNIQUE constraint on (`parent_sched`, `date`); if
    TRUST activated the train in the meantime, we're good.

## Why this works

- This algorithm essentially does the same thing as TRUST when searching for schedules
  -- that is, grab a whole bunch of matching ones and find an authoritative one - but
  with less data, so it returns potentially more schedules than would be valid.
  - All this means is that there's a potential for >1 authoritative schedules, which
    we fail on anyway.
- **NB:** Actually, this doesn't really work. See *Problems* below.

## Problems

- This algorithm is rather conservative - if we can't find a valid ITPS schedule for
  our Darwin train, we fail to do anything with that Darwin train at all.
  - This ends up in lost data - for VSTP trains where the VSTP schedule hasn't come
    through at the time of Darwin activation, and for trains that are potentially
    'Darwin-only' (if such a thing exists).
- This breaks with VSTP schedules that are identical to ITPS schedules apart from
  their differing `source` - a Darwin activation will use the ITPS schedule, while
  the TRUST activation will know about the source change and use the VSTP schedule -
  creating duplicate trains!
- This probably breaks when we have old ITPS schedule data (e.g. in the early morning)
  which hasn't yet been updated, and Darwin goes and uses the old stuff.
  - Well, this is a problem for TRUST as well.
