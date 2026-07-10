import type { CodegenConfig } from '@graphql-codegen/cli'

// The schema is not authored — it is a pure function of the contract
// (docs/spec/graphql.md §1). Two ways to reach it:
//   live:    start `spock run`, then `npm run generate` (introspection);
//   offline: `spock gen graphql-schema app.spock -o schema.graphql`, then
//            SPOCK_SCHEMA=schema.graphql npm run generate.
const config: CodegenConfig = {
  schema: process.env.SPOCK_SCHEMA ?? process.env.SPOCK_URL ?? 'http://127.0.0.1:4000/graphql/v1',
  documents: ['src/**/*.ts'],
  ignoreNoDocuments: true,
  generates: {
    './src/gql/': {
      preset: 'client',
      config: {
        documentMode: 'string',
        // spock's derived scalars serialize as strings on the wire
        scalars: { uuid: 'string', timestamp: 'string' },
      },
    },
  },
}

export default config
