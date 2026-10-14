# Separate Game Modules And Telemetry Adapters

DSCC separates game modules from telemetry adapters: a game module identifies
and presents a supported game, while a telemetry adapter reads a data source and
publishes normalized signals. This keeps shared adapters such as Forza Data Out
from being overloaded as game identity and preserves the distinction between
`moduleId` and `adapterId`.
