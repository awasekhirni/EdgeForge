# EdgeForge: Run Massive AI Models on Your Own Machine
## Copyright Beta ORI Inc. Canada. All Rights Reserved.
## Syed, Awase Khirni www.betaori.com

**The Problem:** Running a trillion-parameter AI model typically requires a data center with racks of GPUs. Most of us don’t have that.

**The Solution:** EdgeForge is a desktop application that lets you run enormous Mixture-of-Experts (MoE) models on a single powerful workstation—no cloud, no subscriptions, no data leaving your machine.

## How It Works

The magic is in smart storage streaming. Instead of loading the entire 2-terabyte model into RAM (impossible for most machines), EdgeForge keeps the **dense core**—the model’s decision-making brain—in memory, while the massive expert pool lives on your NVMe SSD.

When you send a prompt, the system predicts which experts are needed, fetches them at blazing speeds using direct I/O, and streams tokens back to you. Think of it as a library where the librarian already knows which books you’ll want next.

## Who It’s For

- Researchers who need accuracy-critical outputs.
- Developers building on local AI.
- Anyone who wants ChatGPT-level capability without the cloud.

The default **FP16 precision** delivers near-lossless quality; **Q8** and **Q4** presets trade a sliver of accuracy for speed and storage headroom.

## The Honest Part

EdgeForge doesn’t pretend your laptop can run this. You’ll need a **2TB NVMe Gen4 SSD minimum**—and here’s the real talk: FP16 on exactly 2TB leaves zero breathing room.

The app will warn you. It will suggest Q8 or a bigger drive. We’d rather tell you the truth than let you hit a wall mid-download.

## Built for Privacy First

Everything works offline by default. Networking is opt-in. Telemetry is off. Your prompts never leave your machine unless you explicitly enable observability export—and even then, prompt content is never included.

For teams, an optional **QUIC-based mesh** lets multiple machines pool capacity—not speed. We’re honest about that too.

## The Experience

A **Tauri-based desktop app** guides you through hardware detection, model import, and first-run setup.

Developers get an **OpenAI-compatible REST API** on localhost—a drop-in replacement for existing tooling. Python users get prebuilt wheels; no compilation headaches.

## The Philosophy

EdgeForge is built on a simple belief: **powerful AI shouldn’t require permission.**

No API keys, no rate limits, no terms-of-service roulette. Just you, your hardware, and a model that fits on your desk.

## Hardware Requirements

| Spec | Minimum | Recommended |
|---|---|---|
| SSD | 2TB NVMe Gen4 | 4TB NVMe Gen4/Gen5 |
| RAM | 16GB | 32GB |
| CPU | 4 cores | 8+ cores |
| GPU | Optional | Discrete GPU with 16GB+ VRAM |

Welcome to local AI without compromises.
