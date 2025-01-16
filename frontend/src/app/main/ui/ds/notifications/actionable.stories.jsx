import * as React from "react";
import Components from "@target/components";

const { Actionable } = Components;

export default {
  title: "Notifications/Actionable",
  component: Actionable,
  argTypes: {
    title: {
      control: { type: "text" },
    }
  },
  args: {
    title: "Actionable"
  },
  render: ({ ...args }) => <Actionable {...args}/>,
};

export const Default = {};

