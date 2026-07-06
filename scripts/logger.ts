import pino from "pino";

const LOGGING_LEVEL = process.env["PINO_LOG_LEVEL"] || "info";

declare global {
  var logger: pino.Logger | undefined;
}

const transport = {
  targets: [
    { target: "pino-pretty", options: { destination: process.stdout.fd } },
  ],
};

function getLogger() {
  console.log("Starting logger...");

  return pino({
    level: LOGGING_LEVEL,
    timestamp: pino.stdTimeFunctions.isoTime,
    transport,
  });
}

const logger: pino.Logger = global.logger ?? (global.logger = getLogger());

export default logger;
