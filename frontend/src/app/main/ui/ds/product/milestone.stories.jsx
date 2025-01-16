import * as React from "react";
import Components from "@target/components";

const { Milestone } = Components;

export default {
  title: "Product/Milestone",
  component: Milestone,
  argTypes: {
    title: {
      control: { type: "text" },
    }
  },
  args: {
    title: "Milestone"
  },
  render: ({ ...args }) => <Milestone {...args}/>,
};

export const Default = {};

