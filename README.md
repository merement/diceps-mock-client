# diceps-mock-client

## Research Dice-MCU Architecture Emulator

A Rust-based firmware emulator designed to research, develop, and evaluate communication protocols for an MCU-based research Dice architecture.

This repository houses the embedded firmware targeted for the ESP32-C6-WROOM-1 development platform. It acts as a physical hardware-in-the-loop (HIL) model to isolate, prototype, and refine protocol behaviors before the final hardware architecture is finalized.

------------------------------

## 📌 Project Overview

The core purpose of this emulator is to establish a rigorous sandbox for studying host-to-device communication interfaces. The firmware models two distinct operational tracks:

   1. Normal Operation Mode: Simulates standard computational workloads and asynchronous data streams typical of the target architecture.
   2. Interactive/Control Mode: Allows a remote host to interrupt execution, request detailed status reports, audit internal register states, and issue structural commands.

The communication module implemented here serves as the reference design for the production silicon's eventual communication layer.

------------------------------

## 🏗 System Architecture

The research environment relies on a dual-system topology:

+-----------------------+                    +---------------------------------------+

|                       |   USB Serial Port  |          ESP32-C6-WROOM-1             |
|   Host PC Application |<==================>| (Research Architecture Emulator)      |
| (Separate Repository) |   (Wired Link)     |  - Command Parser  - Status Reporting |
|                       |                    |  - Normal/Interactive State Machine   |
+-----------------------+                    +---------------------------------------+


* Host Subsystem: A separate Rust/Cargo utility handling orchestration, interactive CLI workflows, and telemetry logging [diceps-host](https://github.com/merement/diceps-host).

* Target Subsystem (This Repository): Bare-metal firmware parsing commands and executing structural emulation over a physical USB-UART serial link.

------------------------------

## ⚡ Features

* Hardware-in-the-Loop Emulation: Compiles natively for riscv32imac-unknown-none-elf to run bare-metal on ESP32-C6 silicon.

* Serial Protocol Processing: Zero-allocation command parsing engine engineered for highly constrained embedded environments.

* Dynamic State Machine: Smooth context-switching between high-throughput normal data loops and interactive command/control loops.

* Extensible Transport Layer: Architecture isolates the serial driver from the core parsing logic, allowing seamless future adaptation to Wi-Fi or Bluetooth (BLE) transport layers.

------------------------------

## 🚀 Getting Started## Prerequisites

You need the Rust toolchain configured for Espressif RISC-V targets.

   1. Install the RISC-V target package:
   
    rustup target add riscv32imac-unknown-none-elf
   
   2. Install [espflash](https://github.com/esp-rs/espflash) for flashing and monitoring firmware over serial ports:
   
    cargo install espflash
   
   
## Building the Firmware

To compile the firmware binary for the target development board:

    cargo build --release

## Flashing to ESP32-C6

Connect your ESP32-C6-WROOM-1 board to your host computer via a USB cable, then execute:

    cargo run --release

Note: Our configured runner in .cargo/config.toml automatically invokes espflash to flash the chip and open a serial monitor interface.

------------------------------

## 📑 Protocol & Interface Specifications## Serial Link Parameters

* Baud Rate: 115200 (Default)
* Data Bits: 8
* Parity: None
* Stop Bits: 1

## Core Command Set (Examples)

| Command Opcode | Description | Response Payload |
|---|---|---|
| AUTH | TODO | TODO |

------------------------------

## 🔮 Roadmap & Future Research

* Baseline serial communications stack on ESP32-C6.
* Command parsing core and multi-mode state machine execution.
* Integration testing with the upstream Rust Host Application.
* Evaluation of wireless transport layers (Wi-Fi 6 / Bluetooth 5 stacks) using the same parsing engine.
