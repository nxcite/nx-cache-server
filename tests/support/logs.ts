export interface LogLine {
    level: string;
    target: string;
    message?: string;
    span?: Record<string, unknown>;
    [field: string]: unknown;
}

export function jsonLines(stdout: string): LogLine[] {
    return stdout
        .split("\n")
        .filter((line) => line.trim() !== "")
        .map((line) => {
            try {
                return JSON.parse(line) as LogLine;
            } catch (error) {
                throw new Error(`stdout line is not JSON: ${line}`, { cause: error });
            }
        });
}

export function requestLines(stdout: string): LogLine[] {
    return jsonLines(stdout).filter((line) => line.message === "request");
}
