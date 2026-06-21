/**
 * layout.js — Top-down dependency trie layout with taxi-safe routing
 *
 * Uses a Sugiyama-inspired approach:
 *   1. Longest-path layer assignment from root nodes.
 *   2. BFS column assignment: parents spread their children evenly
 *      around the parent's column. First-parent wins for shared children.
 *   3. Compaction pass: shift each layer inward toward parent medians
 *      to reduce edge length while preserving order.
 *   4. Leaf-only nodes pushed to periphery.
 *   5. Overlap resolution.
 *
 * This keeps closely-related chains compact while pushing disconnected
 * or leaf-only nodes to the edges.
 */
function runTopDownLayout(cy, elements) {
    // ---- Collect nodes ----
    var nodeIds = [];
    var nodeSet = {};
    elements.forEach(function (el) {
        if (el.group === 'nodes' && !el.data.ghost) {
            nodeIds.push(el.data.id);
            nodeSet[el.data.id] = true;
        }
    });
    if (nodeIds.length === 0) return;

    // ---- Build adjacency ----
    var outEdges = {};
    var inEdges = {};
    nodeIds.forEach(function (id) {
        outEdges[id] = [];
        inEdges[id] = [];
    });
    elements.forEach(function (el) {
        if (el.group === 'edges') {
            var s = el.data.source;
            var t = el.data.target;
            if (nodeSet[s] && nodeSet[t]) {
                if (outEdges[s].indexOf(t) === -1) outEdges[s].push(t);
                if (inEdges[t].indexOf(s) === -1) inEdges[t].push(s);
            }
        }
    });

    // ---- Step 1: Longest-path layer assignment ----
    var roots = nodeIds.filter(function (id) { return inEdges[id].length === 0; });
    if (roots.length === 0) roots = [nodeIds[0]];

    var layer = {};
    nodeIds.forEach(function (id) { layer[id] = 0; });

    var order = [];
    var visited = {};
    function dfsTopo(id) {
        if (visited[id]) return;
        visited[id] = true;
        outEdges[id].forEach(function (t) { dfsTopo(t); });
        order.push(id);
    }
    roots.forEach(function (r) { dfsTopo(r); });
    nodeIds.forEach(function (id) { if (!visited[id]) order.push(id); });
    order.reverse();

    order.forEach(function (id) {
        outEdges[id].forEach(function (t) {
            if (layer[t] < layer[id] + 1) layer[t] = layer[id] + 1;
        });
    });

    var maxLayer = 0;
    var byLayer = {};
    nodeIds.forEach(function (id) {
        var l = layer[id];
        if (l > maxLayer) maxLayer = l;
        if (!byLayer[l]) byLayer[l] = [];
        byLayer[l].push(id);
    });

    // ---- Step 2: BFS column assignment with offset to prevent parent-child overlap ----
    // Roots centered. Each parent spreads its children around its column,
    // shifted by 0.5 when odd child count so no child lands on parent's column.
    var col = {};

    roots.sort(function (a, b) {
        return (outEdges[b].length + inEdges[b].length) - (outEdges[a].length + inEdges[a].length);
    });
    var rootCenter = -Math.floor((roots.length - 1) / 2);
    roots.forEach(function (id, idx) {
        col[id] = rootCenter + idx;
    });

    var queued = {};
    roots.forEach(function (r) { queued[r] = true; });
    var queue = roots.slice();

    while (queue.length > 0) {
        var cur = queue.shift();
        var children = outEdges[cur] || [];
        var nextChildren = children.filter(function (t) { return layer[t] === layer[cur] + 1; });
        if (nextChildren.length === 0) continue;

        var curCol = col[cur];
        var n = nextChildren.length;
        // Offset: if n is odd, shift by 0.5 so center child doesn't overlap parent
        var offset = (n % 2 === 1) ? 0.5 : 0;
        var halfSpan = (n - 1) / 2 + offset;
        var childStart = curCol - halfSpan;

        nextChildren.forEach(function (child, idx) {
            if (col[child] === undefined) {
                col[child] = childStart + idx;
            }
            if (!queued[child]) {
                queued[child] = true;
                queue.push(child);
            }
        });
    }

    // ---- Step 3: Compaction pass ----
    // For each layer, shift nodes toward parent median to reduce edge length
    // while preserving relative order. Use average of desired and current
    // to avoid over-shifting.
    for (var pass = 0; pass < 3; pass++) {
        for (var l = 0; l <= maxLayer; l++) {
            var nodes = byLayer[l] || [];
            if (nodes.length < 2) continue;

            nodes.sort(function (a, b) { return col[a] - col[b]; });

            // Desired column = median of parents at layer-1
            // For roots (layer 0), desired = current
            var desired = {};
            nodes.forEach(function (id) {
                if (l === 0) {
                    desired[id] = col[id];
                    return;
                }
                var parents = inEdges[id].filter(function (p) { return layer[p] === l - 1; });
                var parentCols = parents.map(function (p) { return col[p]; }).filter(function (c) { return c !== undefined; });
                if (parentCols.length > 0) {
                    parentCols.sort(function (a, b) { return a - b; });
                    desired[id] = parentCols[Math.floor(parentCols.length / 2)];
                } else {
                    desired[id] = col[id];
                }
            });

            // Compact greedy assignment with symmetric centering
            var minCol = -100;
            nodes.forEach(function (id) {
                var cur = col[id];
                var d = desired[id];
                // Blend: 60% toward desired, 40% keep current
                var target = cur + (d - cur) * 0.6;
                var best = Math.max(Math.round(target), minCol + 1);
                col[id] = best;
                minCol = best;
            });
        }
    }

    // ---- Step 4: Push strays to periphery ----
    // Among nodes at the same layer, those that feed into deeper layers
    // (have descendants reaching the bottom) get priority toward the center.
    // Dead-end nodes that don't connect further down get pushed to the
    // right within their layer, while preserving parent-child alignment.
    for (var l = 0; l < maxLayer; l++) {
        var nodes = byLayer[l] || [];
        if (nodes.length < 2) continue;

        // Compute reachability (how deep each node's descendants go)
        var reach = {};
        function computeReach(id, visitedSet) {
            if (reach[id] !== undefined) return reach[id];
            if (visitedSet[id]) return l;
            visitedSet[id] = true;
            var maxReach = l;
            (outEdges[id] || []).forEach(function (t) {
                var tr = computeReach(t, visitedSet);
                if (tr > maxReach) maxReach = tr;
            });
            reach[id] = maxReach;
            return maxReach;
        }
        nodes.forEach(function (id) { computeReach(id, {}); });

        // Identify anchors (reach deeper than their own layer) vs strays
        var anchors = nodes.filter(function (id) {
            var r = reach[id];
            return r !== undefined && r > l;
        });
        var strays = nodes.filter(function (id) {
            var r = reach[id];
            return r === undefined || r <= l;
        });

        if (anchors.length > 0 && strays.length > 0) {
            // Keep anchor columns as-is, push strays to the right
            anchors.sort(function (a, b) { return col[a] - col[b]; });
            strays.sort(function (a, b) { return col[a] - col[b]; });

            var maxAnchor = col[anchors[anchors.length - 1]];
            strays.forEach(function (id, idx) {
                col[id] = maxAnchor + 1 + idx;
            });
        }
    }

    // ---- Handle any unassigned nodes ----
    var nextFree = -100;
    nodeIds.forEach(function (id) {
        if (col[id] === undefined) {
            col[id] = nextFree++;
        }
    });

    // ---- Step 5: Compact columns to sequential integers ----
    var usedCols = {};
    nodeIds.forEach(function (id) { usedCols[col[id]] = true; });
    var sortedCols = Object.keys(usedCols).map(Number).sort(function (a, b) { return a - b; });
    var colMap = {};
    sortedCols.forEach(function (c, idx) { colMap[c] = idx; });
    var totalCols = sortedCols.length;

    // ---- Step 6: Measure and position ----
    var maxW = 0;
    var maxH = 0;
    cy.nodes().forEach(function (node) {
        if (node.data().ghost) return;
        var bb = node.boundingBox();
        if (bb.w > maxW) maxW = bb.w;
        if (bb.h > maxH) maxH = bb.h;
    });

    // Moderate spacing: tight enough to keep related nodes close,
    // wide enough for taxi edges
    var gapX = Math.max(maxW + 35, 160);
    var gapY = maxH + 45;
    var centerX = -((totalCols - 1) * gapX) / 2;

    cy.nodes().forEach(function (node) {
        if (node.data().ghost) return;
        var id = node.data().id;
        var c = colMap[col[id]];
        var l = layer[id] || 0;
        if (c !== undefined) {
            node.position({
                x: centerX + c * gapX,
                y: l * gapY + 20,
            });
        }
    });

    // ---- Step 7: Overlap resolution ----
    var maxIter = 10;
    while (maxIter-- > 0) {
        var resolved = 0;
        var nodes = cy.nodes().filter(function (n) { return !n.data().ghost; });
        for (var i = 0; i < nodes.length; i++) {
            for (var j = i + 1; j < nodes.length; j++) {
                var a = nodes[i];
                var b = nodes[j];
                var pa = a.position();
                var pb = b.position();
                var bbA = a.boundingBox();
                var bbB = b.boundingBox();
                var overlapX = (bbA.w + bbB.w) / 2 - Math.abs(pa.x - pb.x);
                var overlapY = (bbA.h + bbB.h) / 2 - Math.abs(pa.y - pb.y);
                if (overlapX > 3 && overlapY > 3) {
                    if (pa.x >= pb.x) {
                        a.position({ x: pa.x + overlapX + 6, y: pa.y });
                    } else {
                        b.position({ x: pb.x + overlapX + 6, y: pb.y });
                    }
                    resolved++;
                }
            }
        }
        if (resolved === 0) break;
    }

    cy.fit(60);
}