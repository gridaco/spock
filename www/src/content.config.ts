import { defineCollection } from 'astro:content';
import { z } from 'astro/zod';
import { docsSchema } from '@astrojs/starlight/schema';
import { spockDocsLoader } from './content/spock-docs-loader';

export const collections = {
  docs: defineCollection({
    loader: spockDocsLoader({
      repoRoot: new URL('../../', import.meta.url),
    }),
    schema: docsSchema({
      extend: z.object({
        authority: z.enum([
          'normative',
          'governance',
          'decision-record',
          'non-normative',
          'informational',
          'guide',
        ]),
        sourcePath: z.string(),
        legacyRfd: z.boolean().optional(),
        rfdDecision: z.string().optional(),
        rfdImplementation: z.string().optional(),
      }),
    }),
  }),
};
