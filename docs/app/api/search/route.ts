import { docsSource } from '@/app/source';
import { createFromSource } from 'fumadocs-core/search/server';

export const { GET } = createFromSource(docsSource);
