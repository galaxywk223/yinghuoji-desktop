import { use } from "echarts/core";
import { BarChart, LineChart, PieChart } from "echarts/charts";
import {
  DataZoomComponent,
  GridComponent,
  LegendComponent,
  MarkAreaComponent,
  TitleComponent,
  ToolboxComponent,
  TooltipComponent,
} from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
import VChart from "vue-echarts";

const registeredCharts = new Set<string>();

function registerChart(name: string, modules: Parameters<typeof use>[0]) {
  if (registeredCharts.has(name)) {
    return;
  }
  use(modules);
  registeredCharts.add(name);
}

export function registerPieChartModules() {
  registerChart("pie", [
    CanvasRenderer,
    PieChart,
    TitleComponent,
    TooltipComponent,
    LegendComponent,
    ToolboxComponent,
  ]);
}

export function registerBarChartModules() {
  registerChart("bar", [
    CanvasRenderer,
    BarChart,
    GridComponent,
    TooltipComponent,
    DataZoomComponent,
  ]);
}

export function registerLineChartModules() {
  registerChart("line", [
    CanvasRenderer,
    LineChart,
    GridComponent,
    TooltipComponent,
    LegendComponent,
    DataZoomComponent,
    MarkAreaComponent,
  ]);
}

export { VChart };
