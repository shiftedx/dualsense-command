# Separate Profile Kinds

DSCC keeps software profiles, DualSense Edge onboard profiles, and runtime live
effects as separate concepts because they have different owners, lifetimes, and
safety risks. A saved DSCC tuning profile should not imply controller-memory
writes, and a transient telemetry effect should not be confused with an onboard
setting.
