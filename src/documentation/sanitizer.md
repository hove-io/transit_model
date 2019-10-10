# Sanitizer

The sanitizer check for incoherences in the model and also clean up all dangling
objects (for example, a line which is not referred by any route).  This document
explains this process in details.

## Incoherences
This part of the process will check for model incoherences and will raise an
error if one is found.  The first category is about duplicate identifiers:
- if 2 datasets have the same identifier
- if 2 lines have the same identifier
- if 2 stop points have the same identifier
- if 2 stop areas have the same identifier
- if 2 routes have the same identifier
- if 2 vehicle journeys have the same identifier

The second category is about dangling references:
- if a transfer refers a stop which doesn't exist (`from_stop_id` and
  `to_stop_id`)
- if a vehicle journey refers to a route which doesn't exist
- if a vehicle journey refers to a commercial mode which doesn't exist
- if a vehicle journey refers to a dataset which doesn't exist
- if a vehicle journey refers to a company which doesn't exist
- if a vehicle journey refers to a calendar which doesn't exist
- if a line refers to a network which doesn't exist
- if a line refers to a commercial mode which doesn't exist
- if a route refers to a line which doesn't exist
- if a stop point refers to a stop area which doesn't exist
- if a dataset refers to a contributor which doesn't exist

## Dangling objects
After multiple processes applied to a NTFS, some objects might not be referenced
anymore. This part of the process remove all of these objects:
- datasets which are not referenced
- contributors which are not referenced
- companies which are not referenced
- networks which are not referenced
- lines which are not referenced
- routes which are not referenced
- vehicle journeys which are not referenced
- stop points which are not referenced
- stop areas which are not referenced
- services which doesn't contain any date
- geometries which are not referenced
- equipments which are not referenced
- transfers which are not referenced
- frequencies which are not referenced
- physical modes which are not referenced
- commercial modes which are not referenced
- trip properties which are not referenced
- comments which are not referenced
