
module.exports = {
  preset: 'ts-jest',
  testEnvironment: 'jsdom',
  transform: {
    '\\.[jt]sx?$': 'babel-jest',
  },
  setupFilesAfterEnv: ['./tests/setupTests.ts'],
  moduleNameMapper: {
    '^utils/util$': '<rootDir>/src/utils/util.ts',
  },

};