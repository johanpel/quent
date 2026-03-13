import ELK from 'elkjs';
import { useCallback, useEffect, useLayoutEffect, useRef, MouseEvent, type RefObject } from 'react';
import {
  Background,
  ReactFlow,
  ReactFlowProvider,
  useNodesState,
  useEdgesState,
  useReactFlow,
  MarkerType,
  type Node,
  type Edge,
  type OnMoveStart,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { useAtomValue, useSetAtom } from 'jotai';
import type { DAGData } from '@/services/query-plan/types';
import { QueryPlanNode, type QueryPlanNodeData } from '../query-plan/QueryPlanNode';
import {
  selectedNodeIdsAtom,
  selectedOperatorLabelAtom,
  hoveredOperatorIdAtom,
  hoveredOperatorInfoAtom,
  type HoveredOperatorInfo,
} from '@/atoms/dag';
import type { StatValue } from '@/services/query-plan/types';
import { formatWithPrefix } from '@/services/formatters';
import { operatorTypeColor } from '@/services/colors';

const elk = new ELK();

const elkOptions = {
  'elk.algorithm': 'layered',
  'elk.direction': 'DOWN',
  'elk.layered.spacing.nodeNodeBetweenLayers': '50',
  'elk.spacing.nodeNode': '50',
};

// Custom node types for different operations
const nodeTypes = {
  source: QueryPlanNode,
  scan: QueryPlanNode,
  join: QueryPlanNode,
  joinlocal: QueryPlanNode,
  joinpartition: QueryPlanNode,
  filesystemscan: QueryPlanNode,
  aggregate: QueryPlanNode,
  exchange: QueryPlanNode,
  output: QueryPlanNode,
  stage: QueryPlanNode,
  local: QueryPlanNode,
  project: QueryPlanNode,
  filter: QueryPlanNode,
  sort: QueryPlanNode,
  limit: QueryPlanNode,
  union: QueryPlanNode,
  other: QueryPlanNode,
  default: QueryPlanNode,
};

interface DAGProps {
  data: DAGData;
  height?: string;
}

async function calculateLayout(
  nodes: Node<QueryPlanNodeData>[],
  edges: Edge[]
): Promise<{ nodes: Node<QueryPlanNodeData>[]; edges: Edge[] }> {
  const graph = {
    id: 'root',
    layoutOptions: elkOptions,
    children: nodes.map(node => ({
      id: node.id,
      width: 200,
      height: 60,
    })),
    edges: edges.map(edge => ({
      id: edge.id,
      sources: [edge.source],
      targets: [edge.target],
    })),
  };

  const layout = await elk.layout(graph);

  return {
    nodes:
      layout.children?.map((child, i) => ({
        ...nodes[i],
        position: { x: child.x ?? 0, y: child.y ?? 0 },
      })) ?? [],
    edges: edges,
  };
}

const FlowLayout = ({
  data,
  containerRef,
}: {
  data: DAGData;
  containerRef: RefObject<HTMLDivElement | null>;
}) => {
  const [nodes, setNodes, onNodesChange] = useNodesState<Node<QueryPlanNodeData>>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const { fitView, setCenter, getZoom, getViewport } = useReactFlow();
  const setSelectedNodeIds = useSetAtom(selectedNodeIdsAtom);
  const setSelectedOperatorLabel = useSetAtom(selectedOperatorLabelAtom);
  const selectedNodeIds = useAtomValue(selectedNodeIdsAtom);
  const hoveredOperatorId = useAtomValue(hoveredOperatorIdAtom);
  const hasUserInteracted = useRef(false);
  const mouseInside = useRef(false);

  const handleMoveStart = useCallback<OnMoveStart>(event => {
    if (event !== null) {
      hasUserInteracted.current = true;
    }
  }, []);

  // Convert DAGData to ReactFlow format
  const convertToReactFlow = useCallback(() => {
    // Determine which nodes have incoming/outgoing edges
    const nodesWithIncoming = new Set(data.edges.map(e => e.target));
    const nodesWithOutgoing = new Set(data.edges.map(e => e.source));

    const flowNodes: Node<QueryPlanNodeData>[] = data.nodes.map(node => {
      return {
        id: node.id,
        type: node.type,
        data: {
          nodeId: node.id,
          label: node.label,
          operationType: node.type,
          metadata: node.metadata as QueryPlanNodeData['metadata'],
          hasIncoming: nodesWithIncoming.has(node.id),
          hasOutgoing: nodesWithOutgoing.has(node.id),
        },
        style: {
          width: 'auto',
          minWidth: 200,
          background: 'transparent',
          boxShadow: 'none',
          border: 0,
          padding: 0,
        },
        position: { x: 0, y: 0 }, // Will be set by layout
      };
    });

    const flowEdges: Edge[] = data.edges.map(edge => ({
      id: edge.id,
      source: edge.source,
      target: edge.target,
      type: 'smoothstep',
      style: { strokeWidth: 2 },
      markerEnd: {
        type: MarkerType.ArrowClosed,
        width: 24,
        height: 24,
      },
    }));

    return { flowNodes, flowEdges };
  }, [data]);

  const handleNodeClick = useCallback(
    (_event: MouseEvent, node: Node<QueryPlanNodeData>): void => {
      if (selectedNodeIds.has(node.id)) {
        setSelectedNodeIds(new Set());
        setSelectedOperatorLabel(null);
      } else {
        setSelectedNodeIds(new Set([node.id]));
        setSelectedOperatorLabel(node.data.label);
      }
    },
    [selectedNodeIds, setSelectedNodeIds, setSelectedOperatorLabel]
  );

  // Re-fit view when the react-flow container is resized, but only if the user
  // hasn't interacted with the chart (to maintain any focus states applied)
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const observer = new ResizeObserver(() => {
      if (nodes.length > 0 && !hasUserInteracted.current) {
        fitView({ padding: 0.1, minZoom: 0.1 });
      }
    });
    observer.observe(container);
    return () => observer.disconnect();
  }, [containerRef, fitView, nodes.length]);

  // Pan to hovered node only when triggered from outside (table hover)
  // and only if the node is not already visible in the viewport.
  useEffect(() => {
    if (!hoveredOperatorId || mouseInside.current) return;
    const node = nodes.find(n => n.id === hoveredOperatorId);
    if (!node) return;
    const container = containerRef.current;
    if (!container) return;

    const nw = node.measured?.width ?? 200;
    const nh = node.measured?.height ?? 60;
    const { x: vx, y: vy, zoom } = getViewport();
    const cw = container.clientWidth;
    const ch = container.clientHeight;

    // Node bounds in screen space
    const screenLeft = node.position.x * zoom + vx;
    const screenTop = node.position.y * zoom + vy;
    const screenRight = screenLeft + nw * zoom;
    const screenBottom = screenTop + nh * zoom;

    const isVisible = screenLeft >= 0 && screenTop >= 0 && screenRight <= cw && screenBottom <= ch;
    if (isVisible) return;

    const cx = node.position.x + nw / 2;
    const cy = node.position.y + nh / 2;
    setCenter(cx, cy, { zoom: getZoom(), duration: 200 });
  }, [hoveredOperatorId, nodes, setCenter, getZoom, getViewport, containerRef]);

  // Calculate and apply layout
  useLayoutEffect(() => {
    hasUserInteracted.current = false;

    const applyLayout = async () => {
      const { flowNodes, flowEdges } = convertToReactFlow();
      const layoutResult = await calculateLayout(flowNodes, flowEdges);

      setNodes(layoutResult.nodes);
      setEdges(layoutResult.edges);

      // Fit view after layout
      setTimeout(() => fitView({ padding: 0.1, minZoom: 0.1 }), 0);
    };

    applyLayout();
  }, [data, convertToReactFlow, fitView, setNodes, setEdges]);

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      onNodesChange={onNodesChange}
      onEdgesChange={onEdgesChange}
      onNodeClick={handleNodeClick}
      onMoveStart={handleMoveStart}
      onMouseEnter={() => {
        mouseInside.current = true;
      }}
      onMouseLeave={() => {
        mouseInside.current = false;
      }}
      proOptions={{ hideAttribution: true }}
      nodeTypes={nodeTypes}
      fitView
      minZoom={0.1}
      maxZoom={2}
      defaultEdgeOptions={{
        type: 'smoothstep',
        style: { strokeWidth: 2 },
        markerEnd: { type: MarkerType.ArrowClosed, width: 24, height: 24 },
      }}
    >
      <Background />
    </ReactFlow>
  );
};

function isBytesStat(name: string): boolean {
  return name.includes('_bytes') || name.endsWith('_byte') || name.startsWith('bytes_');
}

function formatStatValue(value: StatValue, key: string): string {
  if (typeof value === 'number') {
    if (isBytesStat(key)) return formatWithPrefix(value, 'B', 'Iec', 2);
    return formatWithPrefix(value, '', 'Si', 2);
  }
  if (Array.isArray(value)) return value.join(', ');
  return String(value);
}

const OperatorStatsOverlay = ({ info }: { info: HoveredOperatorInfo }) => (
  <div className="absolute top-2 right-2 z-50 w-72 bg-card border border-border rounded-md shadow-lg p-3 pointer-events-none">
    <div className="flex items-center justify-between">
      <span className="font-semibold text-sm">{info.label}</span>
      <span
        className="text-xs text-white px-1.5 py-0.5 rounded"
        style={{ backgroundColor: operatorTypeColor(info.operationType) }}
      >
        {info.operationType}
      </span>
    </div>
    <div className="text-xs text-muted-foreground font-mono truncate">{info.nodeId}</div>
    {info.stats.length > 0 && (
      <div className="mt-1 border-t pt-1.5">
        <div className="flex flex-col gap-1">
          {info.stats.map(({ key, value }) => (
            <div key={key} className="text-xs flex items-center justify-between">
              <span className="capitalize">{key.replace(/_/g, ' ')}:</span>
              <span className="text-muted-foreground ml-1 font-mono">
                {formatStatValue(value, key)}
              </span>
            </div>
          ))}
        </div>
      </div>
    )}
  </div>
);

export const DAGChart = ({ data, height = '100%' }: DAGProps) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const hoveredInfo = useAtomValue(hoveredOperatorInfoAtom);
  return (
    <div ref={containerRef} style={{ width: '100%', height }} className="relative">
      <ReactFlowProvider>
        <FlowLayout data={data} containerRef={containerRef} />
      </ReactFlowProvider>
      {hoveredInfo && <OperatorStatsOverlay info={hoveredInfo} />}
    </div>
  );
};
