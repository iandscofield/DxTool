import {
  Battery,
  Bluetooth,
  Camera,
  CircuitBoard,
  Cpu,
  FolderOpen,
  HardDrive,
  Keyboard,
  List,
  MemoryStick,
  Monitor,
  MonitorSmartphone,
  Network,
  Plug,
  Printer,
  Speaker,
  Usb,
  Gauge,
  type LucideIcon,
} from "lucide-react";

const map: Record<string, LucideIcon> = {
  monitor: Monitor,
  cpu: Cpu,
  "memory-stick": MemoryStick,
  gpu: Gauge,
  screen: MonitorSmartphone,
  "hard-drive": HardDrive,
  folder: FolderOpen,
  network: Network,
  battery: Battery,
  printer: Printer,
  plug: Plug,
  slots: CircuitBoard,
  usb: Usb,
  speaker: Speaker,
  keyboard: Keyboard,
  camera: Camera,
  bluetooth: Bluetooth,
  list: List,
};

export function CategoryIcon({ name, size = 16 }: { name: string; size?: number }) {
  const Icon = map[name] ?? List;
  return <Icon size={size} strokeWidth={1.8} />;
}
