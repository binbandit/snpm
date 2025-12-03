'use client';

import Link from 'next/link';
import { useState, useEffect } from 'react';
import { 
  ArrowRight, Terminal, Copy, Check, Zap, Lock, Code, Boxes, Shield, Heart,
  Code2, Sparkles, FileText, Users, BookOpen, Github, Package, X
} from 'lucide-react';

export default function HomePage() {
  return (
    <div className="min-h-screen bg-white dark:bg-[#1a1512] transition-colors">
      <main>
        <HeroSection />
        <KeyFeatures />
        <PerformanceSection />
        <CommandShowcase />
        <ComparisonSection />
        <DifferentiatorsSection />
        <CTA />
        <Footer />
      </main>
    </div>
  );
}

function HeroSection() {
  const [copied, setCopied] = useState(false);
  const [acronym, setAcronym] = useState('Suddenly Not Panicking Manager');
  const [version, setVersion] = useState('2025.12.3');

  useEffect(() => {
    // Fetch random acronym on each render
    fetch('/api/acronym')
      .then(res => res.json())
      .then(data => setAcronym(data.acronym))
      .catch(() => setAcronym('Suddenly Not Panicking Manager'));

    // Fetch latest version from GitHub
    fetch('/api/version')
      .then(res => res.json())
      .then(data => setVersion(data.version))
      .catch(() => setVersion('2025.12.3'));
  }, []);

  const handleCopy = () => {
    navigator.clipboard.writeText("npm install -g snpm");
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <section className="relative pt-32 pb-20 overflow-hidden">
      <div className="absolute inset-0 bg-gradient-to-b from-[#f5f1e8] to-[#faf8f4] dark:from-[#1a1512] dark:to-[#251d16]"></div>

      <div className="relative max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="grid lg:grid-cols-2 gap-12 items-center">
          <div className="space-y-8">
            <div className="inline-flex items-center gap-2 bg-teal-50 dark:bg-teal-900/30 text-teal-800 dark:text-teal-300 px-4 py-2 rounded-full border border-teal-200 dark:border-teal-700/50">
              <span className="w-2 h-2 bg-teal-600 dark:bg-teal-500 rounded-full"></span>
              <span className="text-sm">Built with Rust for unmatched speed</span>
            </div>

            <div className="space-y-6">
              <div className="space-y-3">
                <h1 className="text-5xl sm:text-6xl lg:text-7xl tracking-tight text-gray-900 dark:text-[#f5f1e8]">
                  The <span className="italic">speedy</span> way to manage packages
                </h1>
                <p className="text-sm text-gray-500 dark:text-[#c9b89a] italic">{acronym}</p>
              </div>
              <p className="text-xl text-gray-600 dark:text-[#d4c5b0] max-w-xl">
                A drop-in replacement for npm, yarn, and pnpm. Faster installs, simpler codebase, 
                deterministic builds—everything you need, nothing you don't. A serene development experience.
              </p>
            </div>

            <div className="flex flex-wrap gap-4">
              <Link
                href="/docs"
                className="inline-flex items-center bg-gradient-to-r from-teal-600 to-teal-700 hover:from-teal-700 hover:to-teal-800 text-white px-8 py-3 rounded-lg shadow-lg hover:shadow-xl transition-shadow"
              >
                Get Started
                <ArrowRight className="ml-2 h-5 w-5" />
              </Link>
            </div>

            <div className="pt-6">
              <div className="flex items-center gap-2 text-sm text-gray-600 dark:text-[#c9b89a] mb-2">
                <Terminal className="h-4 w-4" />
                <span>Quick install</span>
              </div>
              <div className="bg-white/60 dark:bg-[#2a2118]/80 backdrop-blur-sm border border-[#d4c5b0] dark:border-[#4a3828] rounded-xl p-4 shadow-sm hover:shadow-md transition-shadow relative group">
                <code className="text-gray-900 dark:text-[#f5f1e8]">$ npm install -g snpm</code>
                <button
                  className="absolute top-3 right-3 p-2 rounded-lg text-gray-500 dark:text-[#c9b89a] hover:text-teal-600 dark:hover:text-teal-400 hover:bg-teal-50 dark:hover:bg-[#3a2d1d] transition-all opacity-0 group-hover:opacity-100"
                  onClick={handleCopy}
                  title="Copy to clipboard"
                >
                  {copied ? <Check className="h-4 w-4 text-teal-600 dark:text-teal-400" /> : <Copy className="h-4 w-4" />}
                </button>
              </div>
            </div>
          </div>

          <div className="relative">
            <div className="aspect-[4/3] rounded-2xl overflow-hidden shadow-2xl ring-1 ring-gray-900/10 dark:ring-teal-500/20">
              <img src="/images/snpm-garden-1.png" alt="Tranquil garden representing peaceful development experience" className="w-full h-full object-cover" />
            </div>

            <div className="absolute -bottom-6 -left-6 bg-white/80 dark:bg-[#2a2118]/90 backdrop-blur-md p-6 rounded-xl shadow-xl border border-[#d4c5b0]/50 dark:border-[#4a3828]/60">
              <div className="space-y-2">
                <div className="text-sm text-gray-600 dark:text-[#c9b89a]">Install time</div>
                <div className="text-4xl bg-gradient-to-r from-teal-600 to-teal-700 bg-clip-text text-transparent">1.2s</div>
                <div className="text-xs text-gray-500 dark:text-[#b8a890]">50% faster than pnpm</div>
              </div>
            </div>

            <div className="absolute -top-6 -right-6 bg-white/80 dark:bg-[#2a2118]/90 backdrop-blur-md p-6 rounded-xl shadow-xl border border-[#d4c5b0]/50 dark:border-[#4a3828]/60">
              <div className="space-y-2">
                <div className="text-sm text-gray-600 dark:text-[#c9b89a]">Global cache</div>
                <div className="text-4xl bg-gradient-to-r from-rose-600 to-rose-700 bg-clip-text text-transparent">100%</div>
                <div className="text-xs text-gray-500 dark:text-[#b8a890]">Package reuse</div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

const features = [
  { icon: Zap, title: "Blazing Fast", description: "Global cache, parallel downloads, and smart reuse. Install dependencies in record time.", stats: "50% faster" },
  { icon: Lock, title: "Deterministic Builds", description: "Simple, readable lockfiles ensure identical installs across all environments.", stats: "100% reproducible" },
  { icon: Code, title: "Drop-in Replacement", description: "Familiar commands. Zero learning curve. Works with your existing projects.", stats: "npm compatible" },
  { icon: Boxes, title: "Monorepo Ready", description: "First-class workspace support with shared lockfiles and local package resolution.", stats: "Workspaces ✓" },
  { icon: Shield, title: "Built with Rust", description: "Clean, maintainable codebase. No unsafe code. Easy to audit and contribute.", stats: "Type safe" },
  { icon: Heart, title: "Simple by Design", description: "No clever tricks. Every line has a purpose. Code that's a joy to maintain.", stats: "Contributor friendly" }
];

function KeyFeatures() {
  return (
    <section id="features" className="py-24 bg-[#faf8f4] dark:bg-[#251d16]">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="text-center space-y-4 mb-16">
          <h2 className="text-4xl sm:text-5xl text-gray-900 dark:text-[#f5f1e8]">Why developers choose snpm</h2>
          <p className="text-xl text-gray-600 dark:text-[#d4c5b0] max-w-3xl mx-auto">
            Built from the ground up to be fast, reliable, and easy to use. Everything you loved about npm and pnpm, without the complexity.
          </p>
        </div>

        <div className="grid md:grid-cols-2 lg:grid-cols-3 gap-8">
          {features.map((feature) => {
            const Icon = feature.icon;
            return (
              <div key={feature.title} className="group relative bg-white/60 dark:bg-[#2a2118]/80 backdrop-blur-sm p-8 rounded-2xl border border-[#d4c5b0]/50 dark:border-[#4a3828]/60 hover:border-teal-300 dark:hover:border-teal-600 transition-all hover:shadow-lg">
                <div className="flex items-start justify-between mb-4">
                  <div className="bg-teal-50 dark:bg-teal-900/30 p-3 rounded-xl group-hover:bg-gradient-to-br group-hover:from-teal-600 group-hover:to-teal-700 transition-all">
                    <Icon className="h-6 w-6 text-teal-700 dark:text-teal-400 group-hover:text-white transition-colors" />
                  </div>
                  <span className="text-xs text-gray-600 dark:text-[#c9b89a] bg-[#e8dcc8] dark:bg-[#3a2d1d] px-2 py-1 rounded-full">{feature.stats}</span>
                </div>
                <h3 className="text-xl text-gray-900 dark:text-[#f5f1e8] mb-2">{feature.title}</h3>
                <p className="text-gray-600 dark:text-[#d4c5b0]">{feature.description}</p>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}

function PerformanceSection() {
  const data = [
    { name: 'npm', time: 45.2, color: '#8BA888' },
    { name: 'yarn', time: 38.7, color: '#E8B4A0' },
    { name: 'pnpm', time: 28.4, color: '#F2988A' },
    { name: 'bun', time: 22.1, color: '#D4A574' },
    { name: 'snpm', time: 14.3, color: '#3DB8C4' }
  ];

  return (
    <section id="performance" className="py-24 bg-gradient-to-b from-[#faf8f4] to-[#f5f1e8] dark:from-[#251d16] dark:to-[#1a1512]">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="grid lg:grid-cols-2 gap-12 items-center">
          <div className="space-y-6">
            <div className="inline-flex items-center gap-2 bg-teal-50 dark:bg-teal-900/30 text-teal-800 dark:text-teal-300 px-4 py-2 rounded-full border border-teal-200 dark:border-teal-700/50">
              <span className="text-sm">Benchmark: React app with 300+ dependencies</span>
            </div>
            
            <h2 className="text-4xl sm:text-5xl text-gray-900 dark:text-[#f5f1e8]">Performance that speaks for itself</h2>
            <p className="text-xl text-gray-600 dark:text-[#d4c5b0]">
              Powered by Rust and built for speed. Global caching, parallel downloads, and intelligent reuse mean your team spends less time waiting and more time building.
            </p>

            <div className="space-y-4 pt-4">
              <div className="flex items-center justify-between p-4 bg-white/60 dark:bg-[#2a2118]/80 backdrop-blur-sm rounded-lg border border-[#d4c5b0]/50 dark:border-[#4a3828]/60">
                <div>
                  <div className="text-sm text-gray-600 dark:text-[#c9b89a]">Cold cache install</div>
                  <div className="text-2xl bg-gradient-to-r from-teal-600 to-teal-700 bg-clip-text text-transparent">14.3s</div>
                </div>
                <div className="text-right">
                  <div className="text-sm text-gray-600 dark:text-[#c9b89a]">vs pnpm</div>
                  <div className="text-2xl text-gray-900 dark:text-[#f5f1e8]">50% faster</div>
                </div>
              </div>
              
              <div className="flex items-center justify-between p-4 bg-white/60 dark:bg-[#2a2118]/80 backdrop-blur-sm rounded-lg border border-[#d4c5b0]/50 dark:border-[#4a3828]/60">
                <div>
                  <div className="text-sm text-gray-600 dark:text-[#c9b89a]">Warm cache install</div>
                  <div className="text-2xl bg-gradient-to-r from-teal-600 to-teal-700 bg-clip-text text-transparent">1.2s</div>
                </div>
                <div className="text-right">
                  <div className="text-sm text-gray-600 dark:text-[#c9b89a]">Disk reuse</div>
                  <div className="text-2xl text-gray-900 dark:text-[#f5f1e8]">100%</div>
                </div>
              </div>
            </div>
          </div>

          <PerformanceChart data={data} />
        </div>
      </div>
    </section>
  );
}

const commands = [
  { title: "Install dependencies", command: "snpm install", description: "Fast, deterministic installs from your lockfile" },
  { title: "Add a package", command: "snpm add react@18", description: "Add packages to dependencies with version resolution" },
  { title: "Add dev dependency", command: "snpm add -D typescript", description: "Add packages to devDependencies" },
  { title: "Remove a package", command: "snpm remove lodash", description: "Remove packages and update lockfile automatically" },
  { title: "Run scripts", command: "snpm run build", description: "Execute package.json scripts with local binaries in PATH" },
  { title: "Production install", command: "snpm install --production", description: "Skip devDependencies for production deployments" }
];

function CommandShowcase() {
  return (
    <section className="py-24 bg-[#f5f1e8] dark:bg-[#1a1512]">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="text-center space-y-4 mb-16">
          <h2 className="text-4xl sm:text-5xl text-gray-900 dark:text-[#f5f1e8]">Familiar commands you already know</h2>
          <p className="text-xl text-gray-600 dark:text-[#d4c5b0] max-w-2xl mx-auto">
            Drop-in replacement means zero learning curve. All your favorite npm commands work exactly as expected.
          </p>
        </div>

        <div className="grid md:grid-cols-2 lg:grid-cols-3 gap-6">
          {commands.map((cmd) => (
            <div key={cmd.command} className="group bg-white/60 dark:bg-[#2a2118]/80 backdrop-blur-sm rounded-2xl border border-[#d4c5b0]/50 dark:border-[#4a3828]/60 p-6 hover:shadow-xl hover:border-teal-300 dark:hover:border-teal-600 transition-all flex flex-col">
              <div className="flex items-start justify-between mb-4 flex-1">
                <div className="flex-1">
                  <h3 className="text-lg text-gray-900 dark:text-[#f5f1e8] mb-1">{cmd.title}</h3>
                  <p className="text-sm text-gray-600 dark:text-[#d4c5b0]">{cmd.description}</p>
                </div>
                <Terminal className="h-5 w-5 text-teal-600 dark:text-teal-400 flex-shrink-0 ml-2" />
              </div>
              
              <div className="bg-gray-900 dark:bg-[#1a1512] rounded-lg p-4 group-hover:shadow-lg transition-shadow border dark:border-[#4a3828]/30">
                <code className="text-sm text-teal-400">$ <span className="text-gray-300 dark:text-[#d4c5b0]">{cmd.command}</span></code>
              </div>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

const comparisons = [
  { feature: "Global package cache", snpm: true, npm: false, yarn: true, pnpm: true },
  { feature: "Parallel downloads", snpm: true, npm: false, yarn: true, pnpm: true },
  { feature: "Deterministic installs", snpm: true, npm: true, yarn: true, pnpm: true },
  { feature: "Workspace support", snpm: true, npm: true, yarn: true, pnpm: true },
  { feature: "Readable lockfile (YAML)", snpm: true, npm: false, yarn: true, pnpm: true },
  { feature: "Built with Rust", snpm: true, npm: false, yarn: false, pnpm: false },
  { feature: "Simple codebase", snpm: true, npm: false, yarn: false, pnpm: false },
  { feature: "Install speed (cold cache)", snpm: "14.3s", npm: "45.2s", yarn: "38.7s", pnpm: "28.4s" }
];

function ComparisonSection() {
  return (
    <section id="comparison" className="py-24 bg-gradient-to-b from-[#f5f1e8] to-[#faf8f4] dark:from-[#1a1512] dark:to-[#251d16]">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="text-center space-y-4 mb-16">
          <h2 className="text-4xl sm:text-5xl text-gray-900 dark:text-[#f5f1e8]">How snpm stacks up</h2>
          <p className="text-xl text-gray-600 dark:text-[#d4c5b0] max-w-2xl mx-auto">See how we compare to the most popular package managers</p>
        </div>

        <div className="overflow-x-auto">
          <div className="bg-white/80 dark:bg-[#2a2118]/90 backdrop-blur-sm rounded-2xl shadow-xl border border-[#d4c5b0]/50 dark:border-[#4a3828]/60">
            <table className="w-full">
              <thead>
                <tr className="border-b border-[#d4c5b0]/50 dark:border-[#4a3828]/60 bg-white/40 dark:bg-[#1a1512]/40">
                  <th className="text-left py-4 px-6 text-gray-900 dark:text-[#f5f1e8]">Feature</th>
                  <th className="text-center py-4 px-6 text-white bg-gradient-to-r from-teal-600 to-teal-700">snpm</th>
                  <th className="text-center py-4 px-6 text-gray-600 dark:text-[#c9b89a]">npm</th>
                  <th className="text-center py-4 px-6 text-gray-600 dark:text-[#c9b89a]">yarn</th>
                  <th className="text-center py-4 px-6 text-gray-600 dark:text-[#c9b89a]">pnpm</th>
                </tr>
              </thead>
              <tbody>
                {comparisons.map((item, index) => (
                  <tr key={item.feature} className={index !== comparisons.length - 1 ? "border-b border-[#d4c5b0]/30 dark:border-[#4a3828]/30" : ""}>
                    <td className="py-4 px-6 text-gray-700 dark:text-[#d4c5b0]">{item.feature}</td>
                    <td className="text-center py-4 px-6 bg-teal-50/50 dark:bg-teal-900/20">
                      {typeof item.snpm === 'boolean' ? (
                        item.snpm ? <Check className="h-5 w-5 text-teal-700 dark:text-teal-400 mx-auto" /> : <X className="h-5 w-5 text-gray-400 dark:text-gray-600 mx-auto" />
                      ) : <span className="text-teal-700 dark:text-teal-400">{item.snpm}</span>}
                    </td>
                    <td className="text-center py-4 px-6">
                      {typeof item.npm === 'boolean' ? (
                        item.npm ? <Check className="h-5 w-5 text-gray-500 dark:text-[#b8a890] mx-auto" /> : <X className="h-5 w-5 text-gray-400 dark:text-gray-600 mx-auto" />
                      ) : <span className="text-gray-600 dark:text-[#b8a890]">{item.npm}</span>}
                    </td>
                    <td className="text-center py-4 px-6">
                      {typeof item.yarn === 'boolean' ? (
                        item.yarn ? <Check className="h-5 w-5 text-gray-500 dark:text-[#b8a890] mx-auto" /> : <X className="h-5 w-5 text-gray-400 dark:text-gray-600 mx-auto" />
                      ) : <span className="text-gray-600 dark:text-[#b8a890]">{item.yarn}</span>}
                    </td>
                    <td className="text-center py-4 px-6">
                      {typeof item.pnpm === 'boolean' ? (
                        item.pnpm ? <Check className="h-5 w-5 text-gray-500 dark:text-[#b8a890] mx-auto" /> : <X className="h-5 w-5 text-gray-400 dark:text-gray-600 mx-auto" />
                      ) : <span className="text-gray-600 dark:text-[#b8a890]">{item.pnpm}</span>}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </section>
  );
}

const differentiators = [
  {
    icon: Code2,
    title: "Implementation Simplicity",
    description: "Clean, boring Rust code that's easy to understand and contribute to. No unsafe code, no clever tricks—just straightforward implementation.",
    details: ["Mid-level Rust developers can read the entire codebase", "No macros beyond standard derive", "Self-documenting code structure", "Strong error types with clear messages"]
  },
  {
    icon: Sparkles,
    title: "Performance Without Magic",
    description: "Fast because of smart engineering, not complex hacks. Global store, parallel downloads, and clean node_modules rebuild.",
    details: ["Global cache to avoid redownloading", "Parallel network and disk work", "No virtual store complexity", "Clear control over dev vs prod installs"]
  },
  {
    icon: FileText,
    title: "Lockfile Clarity",
    description: "Human-readable YAML lockfiles that round-trip cleanly with a simple set of types. No mystery about what's installed.",
    details: ["Direct mapping to resolution graph", "Easy to review in code reviews", "Git-friendly diffs", "Deterministic across all platforms"]
  },
  {
    icon: Users,
    title: "Contributor Friendly",
    description: "Built to be maintained. Strict quality bar means the codebase stays clean as it grows.",
    details: ["Single, coherent style throughout", "No comments except where needed", "Easy to audit and reason about", "Low barrier to contribution"]
  }
];

function DifferentiatorsSection() {
  return (
    <section className="py-24 bg-[#faf8f4] dark:bg-[#251d16]">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="text-center space-y-4 mb-16">
          <h2 className="text-4xl sm:text-5xl text-gray-900 dark:text-[#f5f1e8]">What makes snpm different</h2>
          <p className="text-xl text-gray-600 dark:text-[#d4c5b0] max-w-2xl mx-auto">More than just speed. A philosophy of simplicity and reliability.</p>
        </div>

        <div className="grid lg:grid-cols-2 gap-8">
          {differentiators.map((item) => {
            const Icon = item.icon;
            return (
              <div key={item.title} className="group relative p-10 rounded-3xl bg-gradient-to-br from-white/40 to-white/20 dark:from-[#2a2118]/60 dark:to-[#2a2118]/40 backdrop-blur-sm border border-white/50 dark:border-[#4a3828]/60 hover:border-teal-200 dark:hover:border-teal-600 transition-all duration-300">
                <div className="space-y-5">
                  <div className="flex items-center gap-4">
                    <Icon className="h-7 w-7 text-teal-600 dark:text-teal-400" />
                    <h3 className="text-2xl text-gray-900 dark:text-[#f5f1e8]">{item.title}</h3>
                  </div>
                  
                  <p className="text-gray-600 dark:text-[#d4c5b0] leading-relaxed">{item.description}</p>
                  
                  <div className="pt-2 space-y-3 border-t border-[#d4c5b0]/20 dark:border-[#4a3828]/30">
                    {item.details.map((detail) => (
                      <div key={detail} className="flex items-start gap-3 text-sm text-gray-600 dark:text-[#d4c5b0]">
                        <span className="text-teal-500 dark:text-teal-400 mt-0.5">•</span>
                        <span className="leading-relaxed">{detail}</span>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}

function CTA() {
  return (
    <section className="py-24 relative overflow-hidden">
      <div className="absolute inset-0">
        <img src="/images/snpm-garden-2.png" alt="Tranquil garden background" className="w-full h-full object-cover" />
        <div className="absolute inset-0 backdrop-blur-sm bg-gradient-to-br from-teal-700/50 via-teal-600/40 to-cyan-600/50 dark:from-teal-900/70 dark:via-teal-800/60 dark:to-cyan-900/70"></div>
      </div>
      
      <div className="absolute top-0 right-0 w-96 h-96 bg-cyan-500/20 dark:bg-cyan-600/30 rounded-full blur-3xl"></div>
      <div className="absolute bottom-0 left-0 w-96 h-96 bg-teal-800/20 dark:bg-teal-900/40 rounded-full blur-3xl"></div>
      
      <div className="relative max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 text-center space-y-8">
        <h2 className="text-4xl sm:text-5xl text-white">Ready to speed up your workflow?</h2>
        <p className="text-xl text-white/90 max-w-2xl mx-auto">
          Join developers who have already made the switch to faster, more reliable package management
        </p>
        
        <div className="flex flex-wrap justify-center gap-4">
          <Link href="/docs" className="inline-flex items-center bg-white hover:bg-[#f5f1e8] dark:hover:bg-gray-100 text-teal-700 px-8 py-3 rounded-lg shadow-lg">
            <BookOpen className="mr-2 h-5 w-5" />
            Read the Documentation
          </Link>
          <a href="https://github.com/binbandit/snpm" className="inline-flex items-center px-8 py-3 border border-white/30 bg-white/10 hover:bg-white/20 text-white backdrop-blur-sm rounded-lg">
            <Github className="mr-2 h-5 w-5" />
            View on GitHub
            <ArrowRight className="ml-2 h-5 w-5" />
          </a>
        </div>

        <div className="pt-8">
          <div className="inline-block bg-white/10 backdrop-blur-md border border-white/20 rounded-lg px-6 py-3">
            <code className="text-white">npm install -g snpm</code>
          </div>
        </div>

        <div className="pt-8 border-t border-white/20">
          <p className="text-sm text-white/90">Open source and built with ❤️ by the community</p>
        </div>
      </div>
    </section>
  );
}

function Footer() {
  const [version, setVersion] = useState('2025.12.3');

  useEffect(() => {
    fetch('/api/version')
      .then(res => res.json())
      .then(data => setVersion(data.version))
      .catch(() => setVersion('2025.12.3'));
  }, []);

  return (
    <footer className="bg-[#f5f1e8] dark:bg-[#1a1512] border-t border-[#d4c5b0]/50 dark:border-[#4a3828]/50">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
        <div className="grid md:grid-cols-3 gap-8 mb-8">
          <div className="space-y-4">
            <div className="flex items-center gap-2">
              <div className="bg-gradient-to-br from-teal-600 to-teal-700 p-2 rounded-lg">
                <Package className="h-5 w-5 text-white" />
              </div>
              <span className="text-xl text-gray-900 dark:text-[#f5f1e8]">snpm</span>
              <span className="text-xs text-gray-500 dark:text-[#b8a890] bg-[#e8dcc8] dark:bg-[#3a2d1d] px-2 py-1 rounded-full">v{version}</span>
            </div>
            <p className="text-sm text-gray-600 dark:text-[#d4c5b0]">The speedy way to manage packages for modern JavaScript projects.</p>
          </div>

          <div>
            <h3 className="text-gray-900 dark:text-[#f5f1e8] mb-4">Documentation</h3>
            <ul className="space-y-2 text-sm">
              <li><Link href="/docs" className="text-gray-600 dark:text-[#d4c5b0] hover:text-gray-900 dark:hover:text-teal-400 transition-colors">Getting Started</Link></li>
              <li><Link href="/docs" className="text-gray-600 dark:text-[#d4c5b0] hover:text-gray-900 dark:hover:text-teal-400 transition-colors">CLI Reference</Link></li>
              <li><Link href="/docs" className="text-gray-600 dark:text-[#d4c5b0] hover:text-gray-900 dark:hover:text-teal-400 transition-colors">Configuration</Link></li>
              <li><Link href="/docs" className="text-gray-600 dark:text-[#d4c5b0] hover:text-gray-900 dark:hover:text-teal-400 transition-colors">Migration Guide</Link></li>
            </ul>
          </div>

          <div>
            <h3 className="text-gray-900 dark:text-[#f5f1e8] mb-4">Resources</h3>
            <ul className="space-y-2 text-sm">
              <li><a href="https://github.com/binbandit/snpm" className="text-gray-600 dark:text-[#d4c5b0] hover:text-gray-900 dark:hover:text-teal-400 transition-colors">Changelog</a></li>
              <li><a href="https://github.com/binbandit/snpm" className="text-gray-600 dark:text-[#d4c5b0] hover:text-gray-900 dark:hover:text-teal-400 transition-colors">Community</a></li>
              <li><a href="https://github.com/binbandit/snpm" className="text-gray-600 dark:text-[#d4c5b0] hover:text-gray-900 dark:hover:text-teal-400 transition-colors">Contributing</a></li>
            </ul>
          </div>
        </div>

        <div className="pt-8 border-t border-[#d4c5b0]/50 dark:border-[#4a3828]/50 text-center">
          <p className="text-sm text-gray-600 dark:text-[#c9b89a]">© 2025 snpm. MIT Licensed.</p>
        </div>
      </div>
    </footer>
  );
}


function PerformanceChart({ data }: { data: Array<{ name: string; time: number; color: string }> }) {
  // Dynamic import to avoid SSR issues
  const [mounted, setMounted] = useState(false);
  
  useState(() => {
    setMounted(true);
  });

  if (!mounted) {
    return (
      <div className="bg-white/80 dark:bg-[#2a2118]/90 backdrop-blur-sm p-8 rounded-2xl shadow-xl border border-[#d4c5b0]/50 dark:border-[#4a3828]/60">
        <h3 className="text-xl text-gray-900 dark:text-[#f5f1e8] mb-6">Install Time Comparison (seconds)</h3>
        <div className="h-[300px] flex items-center justify-center text-gray-500 dark:text-[#b8a890]">
          Loading chart...
        </div>
      </div>
    );
  }

  // Lazy load recharts
  const { BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Cell } = require('recharts');

  return (
    <div className="bg-white/80 dark:bg-[#2a2118]/90 backdrop-blur-sm p-8 rounded-2xl shadow-xl border border-[#d4c5b0]/50 dark:border-[#4a3828]/60">
      <h3 className="text-xl text-gray-900 dark:text-[#f5f1e8] mb-6">Install Time Comparison (seconds)</h3>
      <ResponsiveContainer width="100%" height={300}>
        <BarChart data={data}>
          <CartesianGrid strokeDasharray="3 3" stroke="#e5e7eb" className="dark:stroke-gray-700" />
          <XAxis dataKey="name" stroke="#6b7280" className="dark:stroke-gray-400" />
          <YAxis stroke="#6b7280" className="dark:stroke-gray-400" />
          <Tooltip 
            contentStyle={{ 
              backgroundColor: '#fff', 
              border: '1px solid #e5e7eb',
              borderRadius: '0.5rem'
            }}
          />
          <Bar dataKey="time" radius={[8, 8, 0, 0]}>
            {data.map((entry, index) => (
              <Cell key={`cell-${index}`} fill={entry.color} />
            ))}
          </Bar>
        </BarChart>
      </ResponsiveContainer>
      <p className="text-xs text-gray-500 dark:text-[#b8a890] text-center mt-4">
        Lower is better. Benchmarked on MacBook Pro M1, Node 20.x
      </p>
    </div>
  );
}
