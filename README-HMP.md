the trainsplorer
================

## What is this?

This is an in-development project that processes Network Rail's [Open Rail Data](https://wiki.openraildata.com/index.php/Main_Page)
feeds - absorbing all sorts of information about train schedules, and keeping a record of current and historic live train data.
The eventual end goal of the project is to link this information into OpenStreetMap geodata, in order to provide cool things like
a live map of train locations (approximately, that is), predictions on level crossing opening & closing times, and the ability
to find out which trains are going to pass any location on the railway at any given time.

More specifically (since the last paragraph was just copied verbatim from the normal README), the code being submitted here is a
rewritten version of the original trainsplorer code (which used to be called 'osm-signal'), which suffered from a number of issues
relating to its poor architecture. With the osm-signal code, the application was written as two monolithic services, `osms-nrod`
and `osms-web`, which handled obtaining and updating live data and showing it to the user, respectively. 

### Problems with the old design

These services connected
*directly* to a PostgreSQL database, which was a somewhat unwieldy way of doing things; the services would frequently step over
each other when trying to edit data, causing PostgreSQL to report deadlocks, and resulting in some very confusing code that was
written to try and get around this problem. (In fact, `osms-nrod` was the principal cause of this; it connected to two different
sources of live data, and ran multiple threads, each with their own database connection, to process the received results).

In addition, neither service made any attempt at optimizing its queries for network latency, meaning that the PostgreSQL server
pretty much *had* to be on the same machine as the two services for it to work acceptably well (this was especially evident
when the database setup program, `osms-db-setup`, would do its daily schedule update, inserting thousands of new schedules
into the database over a rather long expanse of time).

### Proposed new design

The rewrite aimed to address these shortcomings, by moving to a more microservice-oriented architecture: the application
would be split into multiple (currently 7) small microservices, highly modular small programs which perform only one task
and expose a simple HTTP API, to be consumed by other microservices.

Instead of using PostgreSQL, the new design would use SQLite, for multiple reasons:

- A SQLite database, if tuned correctly, can be a better datastore for a program that only wishes to store data locally
  (and not have it be accessed concurrently by other programs). (cf. [Appropriate Uses for SQLite](https://www.sqlite.org/whentouse.html))
  Since the plan was to have services communicate over HTTP instead of connecting directly to a database, this seemed
  ideal, and solved the issue of network latency.
- SQLite databases have the advantage of being contained in a single file. As is employed in the rewrite currently,
  this means one service can download some data from an external source, perform some processing, and then capture
  the result of that processing in a databse file that can be uploaded to shared storage, then downloaded and used
  by other services.

This new architecture would also enable the services to be deployed in [Docker](https://www.docker.com/) containers,
and managed by [Kubernetes](https://kubernetes.io/), a new *container orchestration* system developed in part by Google.
The reasons for this were:

- Under osm-signal, deploying and updating code was annoying and error-prone, since it involed manually logging into
  the production server (!), compiling the code *on* said server (!!), and restarting the services. With containerization,
  the code can be compiled elsewhere, and updated simply by downloading the latest container and restarting.
- Kubernetes allows things to be run on different servers, and automates a fair deal of the deployment for us; you
  simply describe declaratively what you want running, and in what way (using manifest files; look at the `k8s/` directory!), and
  it does all the work to make the running workloads match what you described. (See [this comic](https://cloud.google.com/kubernetes-engine/kubernetes-comic/)
  for a more detailed explanation of what Kubernetes is!)

Essentially, the focus of this rewritten code is to explore using this different architecture, and to evaluate whether
it is indeed an improvement over the old code (we sincerely hope it is, given the amount of time and effort expended!)
See the *Conclusion* section for such an evaluation.

## Data feeds

`trainsplorer` processes data from various different open data sources, which are worth explaining to give you an idea
of why the application works the way it does.

**Network Rail** provide the *ITPS*, *CORPUS* and *TRUST* data sources.

- *ITPS* (the Integrated Train Planning System) is the system used at Network Rail to plan train schedules. The open
  data [SCHEDULE](https://wiki.openraildata.com/index.php?title=SCHEDULE) feed allows users to download the entire
  ITPS train schedule database, which is quite a mammoth task; there are usually thousands of schedules covering
  the whole year and part of next year, including variations to schedules, cancellations on days where schedules
  do not run, etc.
- These schedules are the basis for pretty much the whole program; in essence, 'all' trainsplorer does is download
  these schedules, and retain information about live updates to said schedules.
- *TRUST* ('Train Running on System TOPS') is the (mainframe-based...) system that issues live updates about schedules,
  providing information about live train movements (i.e. "train X just passed through station Y"). It does not, however,
  issue any predictions; it only reports on things which have already happened.
- *CORPUS* is a reference data source that links the various types of railway location code together (more on that
  later).

**National Rail Enquiries** provide the *MSN* and *Darwin* data sources.

- *Darwin* is the UK's main train data platform, integrating data from all sorts of sources (including TRUST, as well as
  signaling data) to provide predictions and information that power pretty much everything train-related, including
  the actual departure boards visible at stations. The *Darwin Push Port*, used by trainsplorer, issues a live feed
  of train predictions and movements.
- trainsplorer integrates the Darwin data with the TRUST and ITPS data provided above (even though you aren't supposed to);
  this is done primarily because Darwin doesn't provide information about some trains, like freight trains (which do not
  show on passenger information boards), which was deemed necessary for the project's eventual goal of level crossing estimation;
  the ITPS feed is also somewhat 'nicer' to work with than Darwin's equivalent.
- *MSN* (Master Station Names) is another piece of reference data that provides station names.

## Data model

In a fashion that hopefully follows from the data feeds description above, trainsplorer's data model is primarily focused
on *schedule* objects, which represent ITPS schedules, and *train* objects, which represent a live running of one particular
train. The movements of a train in a schedule are known as *schedule movement* objects, with the corresponding *train movement*
objects providing live updates.

The process of converting a schedule into a train is known as *activation*, performed by the `tspl-zugfuhrer` service.
This happens either upon receipt of a TRUST Train Activation message, or when some data about the train arrives via Darwin (whichever
is first). During activation, all the schedule movements from the parent schedule are copied, and become train movements; live
updates are then issued as updates to those train movements.

The documentation comments in `tspl-fahrplan/src/types.rs` and `tspl-zugfuhrer/src/types.rs` also explain this, and are worth
looking at!

## Crate descriptions

This bit explains what all the different subfolders (which are mostly Rust crates) are for. They also all have
different German names describing what they do (for some reason).

- `tspl-util` is a utility library imported by all services, providing common functionality like HTTP RPC, logging, and
  configuration.
- `tspl-sqlite` is another utility library, providing some rudimentary ORM / query builder functionality for SQLite
  to make common database tasks easier.
- `tspl-gcs` is yet another utility library which deals with storing and retrieving data in/from Google Cloud Storage.

Actual services:

- `tspl-fahrplan` (German for 'schedule') handles requests for ITPS schedules.
- `tspl-fahrplan-updater` handles actually downloading the ITPS schedules from the daily update feeds, storing the results
  in Google Cloud Storage for `tspl-fahrplan` to make use of.
- `tspl-zugfuhrer` (German for 'train conductor') handles activating trains, and storing live updates to said trains.
- `tspl-nrod` connects to either Network Rail or Darwin, and feeds live data into `tspl-zugfuhrer`. (The service contains
  code for both; which service to connect to is chosen at runtime.)
- `tspl-nennen` (German for 'to name') downloads MSN and CORPUS reference data, creating a database of station names
  that is also uploaded to Google Cloud Storage.
- `tspl-verknupfen` (German for 'to combine') combines movement data from `tspl-fahrplan` and `tspl-zugfuhrer` to create
  *deduplicated movement* objects, combining a train's scheduled time of arrival with its actual time.
- `tspl-web` is a somewhat hacky proof-of-concept frontend to the above services (with half of its UI code copied from
  its predecessor, `osms-web`), demonstrating train movement search functionality.

Areas of interest are probably the first four services in that list, which form the core backend functionality that the
rewrite hoped to implement. (The last few crates, especially the web one, are of somewhat lower quality...)

## Conclusion

Adopting a microservices-based architecture had some benefits, in that it forced me to think about how and where data
would be stored - seeing as every service would have to interact with other services through a defined API surface, instead
of everything accessing one giant SQL database. Using SQLite definitely made the database code a lot simpler in many places,
as a testament to this fact.

However, the rewritten version is not yet obviously better than the older one - primarily due to the fact that `tspl-zugfuhrer`
suffers some performance problems, on account of only being a single service that cannot be scaled past one replica due
to its reliance on a stateful database (while all other services are stateless). Indeed, `tspl-zugfuhrer` seems to be somewhat
pushing the limits of what SQLite can accomplish perfomance-wise, meaning that it might turn out to be necessary to provision
a PostgreSQL database and switch back to a client-server architecture, just for this service. The rest of the services have,
however, benefited greatly from the rewrite, with the code for `tspl-fahrplan` and `tspl-nrod` being both simpler and faster,
thanks to the more modular architecture, as well as being able to think things through afresh whilst rewriting.

It is hoped that the architecture will, however, make extending the current code to support level crossing time estimation
and other additions far easier, as this functionality can be developed as independent microservices that can even run on
separate, more powerful hardware, thanks to Kubernetes. This has, however, not yet been written, and it is likely that
the performance issues in tspl-zugfuhrer would need fixing first.
