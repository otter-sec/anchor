import type { APIRoute } from 'astro'
import { getAllDocs, docHref } from '@/lib/docs'

// Static (build-time) endpoint, needed for the mcp
export const GET: APIRoute = async () => {
  const docs = await getAllDocs()
  return Response.json(
    docs.map((d) => ({
      id: d.id,
      title: d.data.title,
      description: d.data.description ?? '',
      url: docHref(d.id),
      body: d.body ?? '',
    })),
  )
}
