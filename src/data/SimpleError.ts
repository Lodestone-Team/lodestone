// custom error class that's just a string message
export class SimpleError extends Error {
  constructor(message: string) {
    super(message);
  }
}
