import { QueryResourceTree } from '@/components/QueryResourceTree';
import { queryBundleQueryOptions } from '@/hooks/useQueryBundle';
import { queryClient } from '@/lib/queryClient';
import { createFileRoute } from '@tanstack/react-router';
import { QueryBundle } from '~quent/types/QueryBundle';
import type { EntityRef } from '~quent/types/EntityRef';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';
import { OperatorTable } from '@/components/OperatorTable';

export const Route = createFileRoute('/profile/engine/$engineId/query/$queryId/')({
  component: QueryIndex,
  loader: async ({ params }): Promise<QueryBundle<EntityRef>> => {
    const { engineId, queryId } = params;
    // Use ensureQueryData to populate React Query cache (avoids duplicate fetches)
    return await queryClient.ensureQueryData(queryBundleQueryOptions({ engineId, queryId }));
  },
});

function QueryIndex() {
  const queryBundle = Route.useLoaderData();
  const { engineId } = Route.useParams();
  return (
    <Tabs defaultValue="timeline" className="flex flex-col h-full w-full">
      <div className="shrink-0 border-b px-4 py-1">
        <TabsList>
          <TabsTrigger value="timeline">Timeline</TabsTrigger>
          <TabsTrigger value="operators">Operators</TabsTrigger>
        </TabsList>
      </div>
      <TabsContent value="timeline" className="flex-1 min-h-0 mt-0">
        <div className="flex items-center justify-center w-full h-full min-h-[200px]">
          <QueryResourceTree engineId={engineId} queryBundle={queryBundle} />
        </div>
      </TabsContent>
      <TabsContent value="operators" className="flex-1 min-h-0 mt-0">
        <OperatorTable queryBundle={queryBundle} />
      </TabsContent>
    </Tabs>
  );
}
