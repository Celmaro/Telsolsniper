import { createConnection, type Socket } from "node:net";
import type { ParsedTelegramSignal } from "./parser.js";

export class SignalSocketPublisher {
  private socket: Socket | undefined;
  private connecting: Promise<void> | undefined;

  constructor(private readonly socketPath: string) {}

  async connect(): Promise<void> {
    await this.ensureConnected();
  }

  async publish(signal: ParsedTelegramSignal): Promise<void> {
    await this.ensureConnected();
    const socket = this.socket;
    if (!socket || socket.destroyed) {
      throw new Error("Rust signal socket disconnected before write");
    }

    const payload = `${JSON.stringify(signal)}\n`;
    await new Promise<void>((resolve, reject) => {
      socket.write(payload, "utf8", (error?: Error | null) => {
        if (error) reject(error);
        else resolve();
      });
    });
  }

  private async ensureConnected(): Promise<void> {
    if (this.socket && !this.socket.destroyed && this.socket.writable) {
      return;
    }
    if (this.connecting) {
      return this.connecting;
    }

    this.connecting = new Promise<void>((resolve, reject) => {
      const socket = createConnection({ path: this.socketPath });
      const onInitialError = (error: Error): void => {
        socket.destroy();
        reject(error);
      };

      socket.once("error", onInitialError);
      socket.once("connect", () => {
        socket.removeListener("error", onInitialError);
        this.socket = socket;
        socket.on("error", (error) => {
          console.error("Rust signal socket error:", error.message);
        });
        socket.on("close", () => {
          if (this.socket === socket) this.socket = undefined;
        });
        resolve();
      });
    }).finally(() => {
      this.connecting = undefined;
    });

    return this.connecting;
  }
}
