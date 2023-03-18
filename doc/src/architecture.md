# Architecture

The MSR system uses an _event-driven architecture_.

It can be used in a viarity of contexts but typically
it acts as a connector between high level management systems
and low level devices like PLCs, I/O Systems or Sensors and Actuators
(see [context section](architecture/context.md) for more details).

The top-level components of MSR are referred to as _plugins_
that communicate asynchronously by sending and receiving messages through _channels_
(see [messaging section](architecture/messaging.md) for more details).

Application specific _usecases_ are implemented in a separate layer
that coordinates all the messages from the plugins
but also from other more outer layers such as a web interface
(see [components section](architecture/components.md) for more details).
