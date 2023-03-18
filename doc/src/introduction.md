# Introduction

**MSR** is an open source industrial automation toolbox
developed by [slowtec](https://slowtec.de).

The MSR system is written in [Rust](https://rust-lang.org),
a powerful language for creating reliable and performant software.

Here are some key features of the **MSR** system:

- **Lightweight:**
  Applications build with **MSR** are usually quite small.
  For example, a full-featured IIoT application with integrated web server
  and PLC connection can be bundled within a 2-6 MBytes executable.
  Also the memory usage is typically less than 20 MBytes.

- **Easy deployable**:
  A complete automation application can be compiled into a single executable file
  that has no dependencies.
  This allows it to be easily copied to a target machine without worrying about
  shared libraries or other dependencies.

- **Extendable**:
  The MSR system has a plug-in architecture that allows custom code to be combined
  with standard functions such as fieldbus communication or data logging.

- **Hard real-time**:
  The MSR system can be used in use cases where hard real-time is required.
  For example, a dedicated realtime plugin could run in a separate process
  that is managed by a
  [real-time Linux](https://www.osadl.org/Realtime-Linux.projects-realtime-linux.0.html)
  kernel.

- **Ready made plugins**:
  The MSR project offers several commonly used plugins such as the following:

  - **CSV Recording** - Record (cyclic) data within CSV files
  - **Journaling** - Record application specific events
  - **Modbus** - Communicate via Modbus RTU or Modbus TCP with other devices
    (This plugin is currently in development and not open sourced yet)

- **Open Source**:
  The complete MSR system is licensed under either of
  [MIT License](http://opensource.org/licenses/MIT) or
  [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
   at your option.

## Contributing

MSR is free and open source.
You can find the source code on [GitHub](https://github.com/slowtec/msr)
and issues and feature requests can be posted on the [GitHub issue tracker](https://github.com/slowtec/msr/issues).
MSR relies on the community to fix bugs and add features:
if you'd like to contribute, consider opening a [pull request](https://github.com/slowtec/msr/pulls).
